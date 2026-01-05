/**
 * Jules API TypeScript SDK
 *
 * @example
 * ```ts
 * import { JulesClient } from 'spike-lab/lib'
 *
 * const client = new JulesClient({ apiKey: 'your-key' })
 *
 * // List sessions
 * const { sessions } = await client.listSessions()
 *
 * // Help stuck agent
 * await client.sendMessage('session-id', 'Try using the v2 API')
 *
 * // Approve plan
 * await client.approvePlan('session-id')
 * ```
 */

// Client
export { JulesClient } from './client'
export type { JulesClientConfig } from './client'

// Types
export type {
  // Enums
  SessionState,
  ActivityType,
  ActivityOriginator,
  AutomationMode,
  // Core resources
  Source,
  SourceContext,
  Session,
  SessionOutput,
  Activity,
  // Activity artifacts
  ActivityArtifact,
  BashOutput,
  ChangeSet,
  GitPatch,
  PlanGenerated,
  Plan,
  PlanStep,
  ExtractedPatch,
  // Request types
  CreateSessionRequest,
  SendMessageRequest,
  ListOptions,
  // Response types
  ListSessionsResponse,
  ListActivitiesResponse,
  ListSourcesResponse,
  // Error types
  ApiError,
  ApiErrorResponse,
  ApiErrorDetail,
} from './types'

// Errors
export {
  JulesError,
  AuthenticationError,
  ForbiddenError,
  NotFoundError,
  RateLimitError,
} from './errors'

// SPL Integration
export {
  delegateViaJules,
  runSplWithJulesDelegate,
  buildPromptFromSpecPack,
  type SplProject,
  type DelegateResult,
  type DelegateOptions,
} from './spl-jules'
