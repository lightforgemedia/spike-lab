import type {
  Session,
  Activity,
  Source,
  CreateSessionRequest,
  ListSessionsResponse,
  ListActivitiesResponse,
  ListSourcesResponse,
  ListOptions,
  ApiErrorResponse,
  ExtractedPatch,
  BashOutput,
} from './types'
import { JulesError } from './errors'

const BASE_URL = 'https://jules.googleapis.com/v1alpha'

export interface JulesClientConfig {
  /** API key (or set JULES_API_KEY env var) */
  apiKey?: string
  /** Base URL override (for testing) */
  baseUrl?: string
}

/**
 * Jules API Client
 *
 * @example
 * ```ts
 * const client = new JulesClient({ apiKey: 'your-key' })
 * const sessions = await client.listSessions()
 * ```
 */
export class JulesClient {
  private readonly apiKey: string
  private readonly baseUrl: string

  constructor(config: JulesClientConfig = {}) {
    const apiKey = config.apiKey ?? process.env['JULES_API_KEY']
    if (!apiKey) {
      throw new Error(
        'API key required. Pass apiKey in config or set JULES_API_KEY env var.'
      )
    }
    this.apiKey = apiKey
    this.baseUrl = config.baseUrl ?? BASE_URL
  }

  // ============ Private Helpers ============

  private async request<T>(
    method: 'GET' | 'POST' | 'DELETE',
    path: string,
    body?: unknown
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`

    const headers: Record<string, string> = {
      'x-goog-api-key': this.apiKey,
      'Content-Type': 'application/json',
    }

    const init: RequestInit = {
      method,
      headers,
    }

    if (body) {
      init.body = JSON.stringify(body)
    }

    const response = await fetch(url, init)

    const data = await response.json()

    if (!response.ok) {
      throw JulesError.fromResponse(data as ApiErrorResponse)
    }

    return data as T
  }

  private buildQuery(options: ListOptions = {}): string {
    const params = new URLSearchParams()
    if (options.pageSize) params.set('pageSize', String(options.pageSize))
    if (options.pageToken) params.set('pageToken', options.pageToken)
    const query = params.toString()
    return query ? `?${query}` : ''
  }

  // ============ Sources ============

  /**
   * List connected sources (GitHub repos)
   */
  async listSources(options?: ListOptions): Promise<ListSourcesResponse> {
    const query = this.buildQuery(options)
    return this.request<ListSourcesResponse>('GET', `/sources${query}`)
  }

  // ============ Sessions ============

  /**
   * Create a new session
   */
  async createSession(request: CreateSessionRequest): Promise<Session> {
    return this.request<Session>('POST', '/sessions', request)
  }

  /**
   * Get a session by ID
   */
  async getSession(sessionId: string): Promise<Session> {
    return this.request<Session>('GET', `/sessions/${sessionId}`)
  }

  /**
   * List all sessions
   */
  async listSessions(options?: ListOptions): Promise<ListSessionsResponse> {
    const query = this.buildQuery(options)
    return this.request<ListSessionsResponse>('GET', `/sessions${query}`)
  }

  /**
   * Send a message to a session (help stuck agent)
   */
  async sendMessage(sessionId: string, message: string): Promise<void> {
    await this.request<unknown>('POST', `/sessions/${sessionId}:sendMessage`, {
      prompt: message,
    })
  }

  /**
   * Approve a session's plan
   */
  async approvePlan(sessionId: string): Promise<void> {
    await this.request<unknown>('POST', `/sessions/${sessionId}:approvePlan`, {})
  }

  // ============ Activities ============

  /**
   * List activities for a session
   */
  async listActivities(
    sessionId: string,
    options?: ListOptions
  ): Promise<ListActivitiesResponse> {
    const query = this.buildQuery(options)
    return this.request<ListActivitiesResponse>(
      'GET',
      `/sessions/${sessionId}/activities${query}`
    )
  }

  // ============ Convenience Methods ============

  /**
   * Get all sessions (auto-paginate)
   */
  async *iterateSessions(pageSize = 20): AsyncGenerator<Session> {
    let pageToken: string | undefined

    do {
      const opts: ListOptions = { pageSize }
      if (pageToken) opts.pageToken = pageToken

      const response = await this.listSessions(opts)
      for (const session of response.sessions) {
        yield session
      }
      pageToken = response.nextPageToken
    } while (pageToken)
  }

  /**
   * Get all activities for a session (auto-paginate)
   */
  async *iterateActivities(
    sessionId: string,
    pageSize = 20
  ): AsyncGenerator<Activity> {
    let pageToken: string | undefined

    do {
      const opts: ListOptions = { pageSize }
      if (pageToken) opts.pageToken = pageToken

      const response = await this.listActivities(sessionId, opts)
      for (const activity of response.activities) {
        yield activity
      }
      pageToken = response.nextPageToken
    } while (pageToken)
  }

  /**
   * Wait for session to reach a terminal state
   */
  async waitForCompletion(
    sessionId: string,
    options: { pollInterval?: number; timeout?: number } = {}
  ): Promise<Session> {
    const { pollInterval = 5000, timeout = 300000 } = options
    const start = Date.now()

    while (Date.now() - start < timeout) {
      const session = await this.getSession(sessionId)

      if (
        session.state === 'COMPLETED' ||
        session.state === 'FAILED' ||
        session.state === 'CANCELLED'
      ) {
        return session
      }

      await new Promise((resolve) => setTimeout(resolve, pollInterval))
    }

    throw new Error(`Timeout waiting for session ${sessionId} to complete`)
  }

  // ============ Patch Extraction ============

  /**
   * Extract all git patches from a session's activities
   * Returns patches in chronological order (oldest first)
   */
  async extractPatches(sessionId: string): Promise<ExtractedPatch[]> {
    const patches: ExtractedPatch[] = []

    for await (const activity of this.iterateActivities(sessionId, 50)) {
      if (!activity.artifacts) continue

      for (const artifact of activity.artifacts) {
        if (artifact.changeSet?.gitPatch?.unidiffPatch) {
          const extracted: ExtractedPatch = {
            sessionId,
            patch: artifact.changeSet.gitPatch.unidiffPatch,
            activityId: activity.id,
            createdAt: activity.createTime,
          }
          if (artifact.changeSet.gitPatch.baseCommitId) {
            extracted.baseCommitId = artifact.changeSet.gitPatch.baseCommitId
          }
          if (artifact.changeSet.gitPatch.suggestedCommitMessage) {
            extracted.suggestedCommitMessage = artifact.changeSet.gitPatch.suggestedCommitMessage
          }
          patches.push(extracted)
        }
      }
    }

    // Return in chronological order (API returns newest first)
    return patches.reverse()
  }

  /**
   * Get the latest/final patch from a session
   * This is typically the one you want to apply
   */
  async getLatestPatch(sessionId: string): Promise<ExtractedPatch | null> {
    // Fetch recent activities (newest first)
    const response = await this.listActivities(sessionId, { pageSize: 50 })

    for (const activity of response.activities) {
      if (!activity.artifacts) continue

      for (const artifact of activity.artifacts) {
        if (artifact.changeSet?.gitPatch?.unidiffPatch) {
          const extracted: ExtractedPatch = {
            sessionId,
            patch: artifact.changeSet.gitPatch.unidiffPatch,
            activityId: activity.id,
            createdAt: activity.createTime,
          }
          if (artifact.changeSet.gitPatch.baseCommitId) {
            extracted.baseCommitId = artifact.changeSet.gitPatch.baseCommitId
          }
          if (artifact.changeSet.gitPatch.suggestedCommitMessage) {
            extracted.suggestedCommitMessage = artifact.changeSet.gitPatch.suggestedCommitMessage
          }
          return extracted
        }
      }
    }

    return null
  }

  /**
   * Extract bash command outputs from a session's activities
   */
  async extractBashOutputs(sessionId: string): Promise<BashOutput[]> {
    const outputs: BashOutput[] = []

    for await (const activity of this.iterateActivities(sessionId, 50)) {
      if (!activity.artifacts) continue

      for (const artifact of activity.artifacts) {
        if (artifact.bashOutput) {
          outputs.push(artifact.bashOutput)
        }
      }
    }

    return outputs.reverse()
  }

  /**
   * Get the current plan steps from a session
   */
  async getPlan(sessionId: string): Promise<{ id: string; steps: { title: string; index?: number }[] } | null> {
    const response = await this.listActivities(sessionId, { pageSize: 50 })

    for (const activity of response.activities) {
      if (activity.planGenerated?.plan) {
        return activity.planGenerated.plan
      }
    }

    return null
  }

  // ============ Workflow Automation ============

  /**
   * Wait for session, extract patch, and return it ready to apply
   *
   * @example
   * ```ts
   * const result = await client.waitAndExtractPatch(sessionId)
   * if (result.patch) {
   *   // Write to file and apply with git
   *   await Bun.write('/tmp/fix.patch', result.patch.patch)
   *   // Then: git apply /tmp/fix.patch
   * }
   * ```
   */
  async waitAndExtractPatch(
    sessionId: string,
    options: { pollInterval?: number; timeout?: number; onProgress?: (state: string) => void } = {}
  ): Promise<{ session: Session; patch: ExtractedPatch | null }> {
    const { pollInterval = 5000, timeout = 600000, onProgress } = options
    const start = Date.now()

    while (Date.now() - start < timeout) {
      const session = await this.getSession(sessionId)

      if (onProgress) {
        onProgress(session.state)
      }

      // Terminal states
      if (session.state === 'COMPLETED' || session.state === 'FAILED' || session.state === 'CANCELLED') {
        const patch = await this.getLatestPatch(sessionId)
        return { session, patch }
      }

      // Auto-approve if waiting
      if (session.state === 'AWAITING_APPROVAL') {
        await this.approvePlan(sessionId)
        if (onProgress) {
          onProgress('APPROVED')
        }
      }

      await new Promise((resolve) => setTimeout(resolve, pollInterval))
    }

    throw new Error(`Timeout waiting for session ${sessionId}`)
  }
}
