# Jules Agent System Improvements

Learnings and proposed improvements from orchestrating Jules test generation sessions.

## Session Statistics

| Session | Project | Duration | Iterations | Outcome | Tokens (monitor) |
|---------|---------|----------|------------|---------|------------------|
| 6121528421249549113 | Orchestrator | 9.6 hours | 116 | 3 tests | ~12M |
| 17298667440993899241 | Orchestrator | ~1 hour | 12 | 5 tests | N/A |
| 13382424844987770543 | SPL | ~3 hours | 30 | 7 tests | N/A |

**Insight**: Old session (without pit-of-success) took 10x longer and produced fewer tests.

---

## Key Learnings

### 1. Failure Loops Are Expensive
Jules will retry the same failing approach indefinitely. The old orchestrator session ran 43 consecutive `cargo test` failures before asking for help.

**Cost**: Each iteration consumes API tokens. 116 iterations × ~100k tokens = ~12M tokens wasted on monitoring alone.

### 2. Generic Prompts Fail
"Add tests to this project" is too vague. Jules needs:
- Explicit file paths (`crates/core/tests/core_tests.rs`)
- Working examples to copy
- What NOT to do (anti-patterns)
- Testability notes (which crates are hard to test)

### 3. Some Code Is Untestable Without Infrastructure
The `daemon` crate requires a running SurrealDB instance. Jules wasted hours trying to test it. Should have been marked "SKIP" upfront.

### 4. Plans Don't Update After Guidance
After sending "don't add inline tests to service.rs", the plan still said "create a tests module in service.rs". Plan review before approval is critical.

### 5. Patch Extraction Is Messy
Jules produces multiple overlapping patches (30+ per session). The "latest" patch is often incomplete. Need consolidation logic.

---

## Proposed Improvements

### 1. Auto-Abandon Stuck Sessions

```typescript
// In monitoring loop
if (consecutiveFailures >= 3) {
  await client.sendMessage(sessionId, "STOP - 3 consecutive failures. Explain the error.")
}
if (consecutiveFailures >= 10) {
  console.log("Session stuck. Abandoning.")
  return { status: 'abandoned', reason: 'stuck_in_failure_loop' }
}
```

**Implementation**: Add to `lib/cli.ts` monitoring logic.

### 2. Pre-Flight Testability Check

Before creating a session, check which modules are testable:

```typescript
interface TestabilityReport {
  crate: string
  testable: 'yes' | 'hard' | 'no'
  reason?: string  // "requires SurrealDB", "needs network mocks"
}

async function checkTestability(projectPath: string): Promise<TestabilityReport[]> {
  // Check for DB dependencies, network calls, external services
  // Return report that gets included in prompt
}
```

**Implementation**: Add to `lib/project-profiler.ts`.

### 3. Plan Review Gate

Before approving a plan, check for known-bad patterns:

```typescript
const BAD_PATTERNS = [
  { pattern: /tests module in.*\.rs/, message: "Don't add inline tests to service files" },
  { pattern: /mod tests.*lib\.rs/, message: "Integration tests don't need mod tests in lib.rs" },
]

async function reviewPlan(sessionId: string): Promise<{ ok: boolean; issues: string[] }> {
  const plan = await client.getPlan(sessionId)
  const issues = BAD_PATTERNS
    .filter(p => plan.steps.some(s => p.pattern.test(s.title)))
    .map(p => p.message)
  return { ok: issues.length === 0, issues }
}
```

**Implementation**: Add `review-plan` command to CLI.

### 4. Patch Consolidation

Extract the final state of each file from overlapping patches:

```typescript
async function consolidatePatches(sessionId: string): Promise<string> {
  const patches = await client.extractPatches(sessionId)
  const fileStates = new Map<string, string>()

  // Apply patches in order, keeping final state of each file
  for (const patch of patches) {
    const files = parsePatchFiles(patch.patch)
    for (const [path, content] of files) {
      fileStates.set(path, content)
    }
  }

  // Generate consolidated patch
  return generatePatch(fileStates)
}
```

**Implementation**: Add `consolidate-patches` command to CLI.

### 5. Cost Tracking

Track token usage per session:

```typescript
interface SessionCost {
  sessionId: string
  promptTokens: number
  completionTokens: number
  monitorTokens: number
  estimatedCost: number
}

// Log after each session
console.log(`Session ${id} cost: ~$${cost.toFixed(2)} (${cost.totalTokens} tokens)`)
```

**Implementation**: Add to session archive JSON.

### 6. Smarter Monitoring

Instead of polling every 5 minutes, detect patterns:

```typescript
async function smartMonitor(sessionId: string) {
  let lastBashCount = 0
  let stuckIterations = 0

  while (true) {
    const outputs = await client.extractBashOutputs(sessionId)
    const failCount = outputs.filter(o => o.exitCode === 101).length

    // Detect stuck pattern: same commands failing repeatedly
    if (outputs.length === lastBashCount && failCount > 3) {
      stuckIterations++
      if (stuckIterations >= 2) {
        await client.sendMessage(sessionId, "You appear stuck. What's the error?")
      }
    } else {
      stuckIterations = 0
    }

    lastBashCount = outputs.length
    await sleep(60_000) // Check every minute when active
  }
}
```

**Implementation**: Replace simple polling in monitor agent.

---

## Priority Order

| Priority | Improvement | Effort | Impact |
|----------|-------------|--------|--------|
| P0 | Auto-abandon stuck sessions | Low | High (saves tokens) |
| P0 | Plan review gate | Low | High (prevents waste) |
| P1 | Testability check | Medium | High (prevents impossible tasks) |
| P1 | Patch consolidation | Medium | Medium (cleaner patches) |
| P2 | Smart monitoring | Medium | Medium (faster detection) |
| P2 | Cost tracking | Low | Low (visibility) |

---

## Metrics to Track

1. **Session success rate**: Completed with usable output / Total sessions
2. **Time to first useful output**: Minutes from creation to first passing test
3. **Iterations per session**: Lower is better
4. **Token cost per test**: Total tokens / Number of tests generated
5. **Guidance messages sent**: Lower is better (means prompts are clearer)

---

## Next Steps

1. [ ] Implement auto-abandon in CLI
2. [ ] Add plan review before approval
3. [ ] Add testability check to profiler
4. [ ] Track session costs in archives
5. [ ] Build dashboard for session metrics
