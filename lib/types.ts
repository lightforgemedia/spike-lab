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
  /** GitHub-specific context (required for create) */
  githubRepoContext?: {
    /** Starting branch for the session */
    startingBranch?: string
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

// ============ Activity Artifacts ============

export interface BashOutput {
  command: string
  output?: string
  exitCode?: number
}

export interface GitPatch {
  unidiffPatch: string
  baseCommitId?: string
  suggestedCommitMessage?: string
}

export interface ChangeSet {
  source: string
  gitPatch?: GitPatch
}

export interface ActivityArtifact {
  bashOutput?: BashOutput
  changeSet?: ChangeSet
}

export interface PlanStep {
  id: string
  title: string
  index?: number
}

export interface Plan {
  id: string
  steps: PlanStep[]
}

export interface PlanGenerated {
  plan: Plan
}

export interface Activity {
  /** Resource name: sessions/{sessionId}/activities/{activityId} */
  name: string
  /** Activity ID */
  id: string
  /** Who originated this activity */
  originator: ActivityOriginator
  /** Activity content */
  content?: string
  /** Activity summary */
  summary?: string
  /** Creation timestamp (ISO 8601) */
  createTime: string
  /** Artifacts (bash outputs, patches) */
  artifacts?: ActivityArtifact[]
  /** Plan generated (if planGenerated activity) */
  planGenerated?: PlanGenerated
  /** Progress update marker */
  progressUpdated?: Record<string, unknown>
  /** Plan approved marker */
  planApproved?: Record<string, unknown>
}

// ============ Extracted Patch ============

export interface ExtractedPatch {
  /** Session ID */
  sessionId: string
  /** The unified diff patch content */
  patch: string
  /** Base commit ID */
  baseCommitId?: string
  /** Suggested commit message */
  suggestedCommitMessage?: string
  /** Activity ID that contained this patch */
  activityId: string
  /** When this patch was created */
  createdAt: string
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
