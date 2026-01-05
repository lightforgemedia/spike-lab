/**
 * SPL + Jules Integration
 *
 * This module connects the SPL pipeline with Jules for AI-powered code generation.
 * Jules handles the "delegate" gate - receiving a spec pack and producing code changes.
 *
 * Workflow:
 * 1. SPL compiles spec pack → revision
 * 2. This integration sends spec to Jules as a task prompt
 * 3. Jules generates code changes and produces a patch
 * 4. Integration extracts patch and applies to SPL workspace
 * 5. SPL continues with audit → validate → land gates
 */

import { JulesClient, type ExtractedPatch } from './index'
import { $ } from 'bun'

export interface SplProject {
  repoRoot: string
  source: string // e.g., "sources/github/owner/repo"
  branch?: string
}

export interface DelegateResult {
  sessionId: string
  sessionUrl: string
  patch: ExtractedPatch | null
  patchFile: string | null
  applied: boolean
  error?: string
}

export interface DelegateOptions {
  /** Jules API key */
  apiKey?: string
  /** Timeout in ms (default: 10 minutes) */
  timeout?: number
  /** Poll interval in ms (default: 5 seconds) */
  pollInterval?: number
  /** Auto-approve Jules plan (default: true) */
  autoApprove?: boolean
  /** Progress callback */
  onProgress?: (state: string, message?: string) => void
}

/**
 * Build a Jules prompt from an SPL spec pack
 */
export function buildPromptFromSpecPack(specPackPath: string): string {
  const specContent = Bun.file(specPackPath).text()

  return `You are implementing a code change according to the following SPL spec pack.

SPEC PACK:
\`\`\`yaml
${specContent}
\`\`\`

INSTRUCTIONS:
1. Read the spec pack carefully - understand the intent, scope, and behavior contracts
2. Only modify files listed in scope.in
3. Do NOT modify files listed in scope.out
4. Follow the use cases exactly - they define expected behavior
5. Ensure behavior contracts are satisfied - inputs produce expected outputs
6. Run the acceptance tests specified before considering the task complete

CONSTRAINTS:
- Do NOT run git commit, git push, or any VCS operations
- Do NOT modify SPL state or approve anything
- Focus only on implementing the code changes
- Create a clean, minimal diff that satisfies the spec

Begin by reading the relevant code files, then implement the changes.`
}

/**
 * Execute the delegate gate via Jules
 *
 * This is the core integration: sends a spec pack to Jules, waits for completion,
 * and returns the patch ready for SPL's audit/validate/land gates.
 */
export async function delegateViaJules(
  project: SplProject,
  specPackPath: string,
  options: DelegateOptions = {}
): Promise<DelegateResult> {
  const {
    apiKey,
    timeout = 600000,
    pollInterval = 5000,
    autoApprove = true,
    onProgress,
  } = options

  const clientConfig: { apiKey?: string } = {}
  if (apiKey) {
    clientConfig.apiKey = apiKey
  }
  const client = new JulesClient(clientConfig)
  const prompt = await buildPromptFromSpecPack(specPackPath)

  // Extract task ID from spec pack for session title
  const specContent = await Bun.file(specPackPath).text()
  const taskMatch = specContent.match(/^task:\s*["']?([^"'\n]+)["']?/m)
  const intentMatch = specContent.match(/^intent:\s*["']?([^"'\n]+)["']?/m)
  const taskId = taskMatch?.[1] ?? 'unknown'
  const intent = intentMatch?.[1] ?? 'SPL delegate task'

  onProgress?.('CREATING', `Creating Jules session for ${taskId}`)

  // Create Jules session
  const sourceContext: { source: string; githubRepoContext?: { startingBranch: string } } = {
    source: project.source,
  }
  if (project.branch) {
    sourceContext.githubRepoContext = { startingBranch: project.branch }
  }
  const session = await client.createSession({
    prompt,
    sourceContext,
    title: `[SPL] ${taskId}: ${intent}`,
  })

  onProgress?.('CREATED', `Session ${session.id} created`)

  const result: DelegateResult = {
    sessionId: session.id,
    sessionUrl: session.url,
    patch: null,
    patchFile: null,
    applied: false,
  }

  try {
    // Wait for completion and extract patch
    const { session: finalSession, patch } = await client.waitAndExtractPatch(
      session.id,
      {
        timeout,
        pollInterval,
        onProgress: (state) => onProgress?.(state),
      }
    )

    if (finalSession.state !== 'COMPLETED') {
      result.error = `Session ended with state: ${finalSession.state}`
      return result
    }

    if (!patch) {
      result.error = 'No patch produced by Jules'
      return result
    }

    result.patch = patch

    // Write patch to file
    const patchFile = `${project.repoRoot}/.spl/patches/${taskId}-${session.id}.patch`
    await $`mkdir -p ${project.repoRoot}/.spl/patches`
    await Bun.write(patchFile, patch.patch)
    result.patchFile = patchFile

    onProgress?.('EXTRACTED', `Patch written to ${patchFile}`)

    // Apply patch (check first)
    const checkResult = await $`git apply --check ${patchFile}`.cwd(project.repoRoot).quiet().nothrow()

    if (checkResult.exitCode !== 0) {
      result.error = `Patch check failed: ${checkResult.stderr.toString()}`
      return result
    }

    await $`git apply ${patchFile}`.cwd(project.repoRoot)
    result.applied = true

    onProgress?.('APPLIED', 'Patch applied successfully')

    return result
  } catch (err) {
    result.error = err instanceof Error ? err.message : String(err)
    return result
  }
}

/**
 * Full SPL + Jules workflow:
 * 1. Compile spec pack
 * 2. Delegate to Jules
 * 3. Return control to SPL for remaining gates
 */
export async function runSplWithJulesDelegate(
  project: SplProject,
  specPackPath: string,
  options: DelegateOptions = {}
): Promise<{
  compileResult?: { taskId: string; revisionId: string }
  delegateResult: DelegateResult
  nextSteps: string[]
}> {
  const { onProgress } = options

  // Step 1: Compile spec pack (if SPL is initialized)
  let compileResult: { taskId: string; revisionId: string } | undefined

  const splDbExists = await Bun.file(`${project.repoRoot}/.spl/spl.db`).exists()

  if (splDbExists) {
    onProgress?.('COMPILING', 'Compiling spec pack via SPL')

    // Extract task ID from spec pack
    const specContent = await Bun.file(specPackPath).text()
    const taskMatch = specContent.match(/^task:\s*["']?([^"'\n]+)["']?/m)
    const taskId = taskMatch?.[1] ?? 'task'

    // Compile via SPL CLI
    const compileCmd = await $`cargo run -p spl-cli -- spec-compile --task ${taskId} --spec ${specPackPath}`
      .cwd(project.repoRoot)
      .quiet()
      .nothrow()

    if (compileCmd.exitCode === 0) {
      const output = compileCmd.stdout.toString()
      const revMatch = output.match(/revision (\S+)/)
      if (revMatch?.[1]) {
        compileResult = { taskId, revisionId: revMatch[1] }
      }
    }
  }

  // Step 2: Delegate to Jules
  const delegateResult = await delegateViaJules(project, specPackPath, options)

  // Step 3: Determine next steps
  const nextSteps: string[] = []

  if (delegateResult.applied) {
    nextSteps.push('Run SPL audit gate: spl worker-run --gate audit')
    nextSteps.push('Run SPL validate gate: spl worker-run --gate validate')
    nextSteps.push('Run SPL post_smoke gate: spl worker-run --gate post_smoke')
    nextSteps.push('Complete landing: spl worker-run --gate land')
  } else if (delegateResult.patchFile) {
    nextSteps.push(`Review patch: cat ${delegateResult.patchFile}`)
    nextSteps.push(`Apply manually: git apply ${delegateResult.patchFile}`)
  } else {
    nextSteps.push(`Check Jules session: ${delegateResult.sessionUrl}`)
    if (delegateResult.error) {
      nextSteps.push(`Error: ${delegateResult.error}`)
    }
  }

  const result: {
    compileResult?: { taskId: string; revisionId: string }
    delegateResult: DelegateResult
    nextSteps: string[]
  } = { delegateResult, nextSteps }

  if (compileResult) {
    result.compileResult = compileResult
  }

  return result
}
