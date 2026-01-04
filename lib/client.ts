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
}
