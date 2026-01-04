/**
 * Jules API TypeScript Types
 * API Version: v1alpha
 */

// ============ Enums ============

export type SessionState =
  | 'STATE_UNSPECIFIED'
  | 'PLANNING'
  | 'AWAITING_APPROVAL'
  | 'EXECUTING'
  | 'COMPLETED'
  | 'FAILED'
  | 'CANCELLED'

export type ActivityType =
  | 'ACTIVITY_TYPE_UNSPECIFIED'
  | 'planGenerated'
  | 'planApproved'
  | 'progressUpdated'
  | 'sessionCompleted'
  | 'messageSent'
  | 'messageReceived'

export type ActivityOriginator = 'agent' | 'user'

export type AutomationMode = 'AUTOMATION_MODE_UNSPECIFIED' | 'AUTO_CREATE_PR'

// ============ Core Resources ============

export interface Source {
  /** Resource name: sources/github/{owner}/{repo} */
  name: string
  /** Display name */
  displayName?: string
}

export interface SourceContext {
  /** Source resource name */
  source: string
  /** Optional branch */
  branch?: string
  /** GitHub-specific context */
  githubRepoContext?: {
    owner: string
    repo: string
    branch?: string
  }
}

export interface SessionOutput {
  /** Pull request URL if created */
  pullRequestUrl?: string
  /** Branch name */
  branch?: string
  /** Commit SHA */
  commitSha?: string
}

export interface Session {
  /** Resource name: sessions/{id} */
  name: string
  /** Session ID */
  id: string
  /** Session title */
  title: string
  /** Initial prompt */
  prompt: string
  /** Current state */
  state: SessionState
  /** Source context */
  sourceContext: SourceContext
  /** Creation timestamp (ISO 8601) */
  createTime: string
  /** Last update timestamp (ISO 8601) */
  updateTime: string
  /** Web UI URL */
  url: string
  /** Session outputs (PRs, commits) */
  outputs?: SessionOutput[]
  /** Whether plan approval is required */
  requirePlanApproval?: boolean
}

export interface Activity {
  /** Resource name: sessions/{sessionId}/activities/{activityId} */
  name: string
  /** Activity type */
  activityType: ActivityType
  /** Who originated this activity */
  originator: ActivityOriginator
  /** Activity content */
  content?: string
  /** Activity summary */
  summary?: string
  /** Creation timestamp (ISO 8601) */
  createTime: string
}

// ============ Request Types ============

export interface CreateSessionRequest {
  /** Task description */
  prompt: string
  /** Source context (repo + branch) */
  sourceContext: SourceContext
  /** Optional title */
  title?: string
  /** Automation mode */
  automationMode?: AutomationMode
  /** Require explicit plan approval */
  requirePlanApproval?: boolean
}

export interface SendMessageRequest {
  /** Message to send */
  prompt: string
}

export interface ListOptions {
  /** Page size (default: 20) */
  pageSize?: number
  /** Page token for pagination */
  pageToken?: string
}

// ============ Response Types ============

export interface ListSessionsResponse {
  sessions: Session[]
  nextPageToken?: string
}

export interface ListActivitiesResponse {
  activities: Activity[]
  nextPageToken?: string
}

export interface ListSourcesResponse {
  sources: Source[]
  nextPageToken?: string
}

// ============ Error Types ============

export interface ApiErrorDetail {
  '@type': string
  reason?: string
  domain?: string
  metadata?: Record<string, string>
}

export interface ApiError {
  code: number
  message: string
  status: string
  details?: ApiErrorDetail[]
}

export interface ApiErrorResponse {
  error: ApiError
}
