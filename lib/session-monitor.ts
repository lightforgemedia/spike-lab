#!/usr/bin/env bun
/**
 * Smart Jules Session Monitor
 *
 * Features:
 * - Detects stuck sessions (repeated failures)
 * - Exponential backoff when idle
 * - Notifies on completion or stuck
 * - Writes status to file for external polling
 *
 * Usage:
 *   bun run lib/session-monitor.ts <session-id> [--output status.json]
 */

import { JulesClient } from './index'

interface MonitorState {
  sessionId: string
  startTime: string
  lastCheck: string
  state: string
  iterations: number
  consecutiveFailures: number
  totalBashCommands: number
  lastBashCount: number
  stuckDetected: boolean
  completed: boolean
  needsAttention: boolean
  message: string
}

interface MonitorConfig {
  sessionId: string
  outputFile?: string
  minIntervalMs: number      // Start with this interval
  maxIntervalMs: number      // Max backoff interval
  stuckThreshold: number     // Consecutive failures before "stuck"
  maxIterations: number      // Give up after this many checks
}

const DEFAULT_CONFIG: Omit<MonitorConfig, 'sessionId'> = {
  minIntervalMs: 30_000,     // 30 seconds
  maxIntervalMs: 300_000,    // 5 minutes max
  stuckThreshold: 5,
  maxIterations: 100,
}

async function loadApiKey(): Promise<string> {
  const configPath = `${process.env.HOME}/.spike-lab`
  try {
    const configFile = Bun.file(configPath)
    const configText = await configFile.text()
    const match = configText.match(/JULES_API_KEY="([^"]+)"/)
    if (match?.[1]) return match[1]
  } catch { /* fall through */ }

  const envKey = process.env['JULES_API_KEY']
  if (envKey) return envKey

  throw new Error('No API key found. Run: spike api-setup')
}

async function monitor(config: MonitorConfig): Promise<MonitorState> {
  const apiKey = await loadApiKey()
  const client = new JulesClient({ apiKey })

  const state: MonitorState = {
    sessionId: config.sessionId,
    startTime: new Date().toISOString(),
    lastCheck: '',
    state: 'unknown',
    iterations: 0,
    consecutiveFailures: 0,
    totalBashCommands: 0,
    lastBashCount: 0,
    stuckDetected: false,
    completed: false,
    needsAttention: false,
    message: 'Starting monitor...',
  }

  let currentInterval = config.minIntervalMs
  let lastState = ''

  const writeStatus = async () => {
    if (config.outputFile) {
      await Bun.write(config.outputFile, JSON.stringify(state, null, 2))
    }
  }

  const notify = (message: string) => {
    state.message = message
    state.needsAttention = true
    console.log(`\n🔔 ATTENTION: ${message}`)
    // Could add OS notification here: osascript, notify-send, etc.
  }

  console.log(`📡 Monitoring session ${config.sessionId}`)
  console.log(`   Output: ${config.outputFile || 'stdout only'}`)
  console.log(`   Stuck threshold: ${config.stuckThreshold} consecutive failures`)
  console.log('')

  while (state.iterations < config.maxIterations) {
    state.iterations++
    state.lastCheck = new Date().toISOString()

    try {
      // Get session status
      const session = await client.getSession(config.sessionId)
      state.state = session.state

      // Check for completion
      if (session.state === 'COMPLETED' || session.state === 'FAILED') {
        state.completed = true
        notify(`Session ${session.state}`)
        await writeStatus()
        return state
      }

      // Get bash outputs to detect failures
      const outputs = await client.extractBashOutputs(config.sessionId)
      state.totalBashCommands = outputs.length

      // Count recent failures (exit code 101 = compilation error)
      const recentOutputs = outputs.slice(-10)
      const recentFailures = recentOutputs.filter(o => o.exitCode === 101).length

      // Detect if stuck (same command count + high failure rate)
      if (outputs.length === state.lastBashCount && recentFailures >= 3) {
        state.consecutiveFailures++
      } else if (outputs.length > state.lastBashCount) {
        // Progress made, reset failure count but check new failures
        const newOutputs = outputs.slice(state.lastBashCount)
        const newFailures = newOutputs.filter(o => o.exitCode === 101).length
        if (newFailures === newOutputs.length && newOutputs.length >= 3) {
          state.consecutiveFailures += newFailures
        } else {
          state.consecutiveFailures = 0
          // Decrease interval on progress
          currentInterval = Math.max(config.minIntervalMs, currentInterval / 2)
        }
      }

      state.lastBashCount = outputs.length

      // Check stuck threshold
      if (state.consecutiveFailures >= config.stuckThreshold && !state.stuckDetected) {
        state.stuckDetected = true
        notify(`Session appears STUCK - ${state.consecutiveFailures} consecutive failures`)
        await writeStatus()
        return state
      }

      // Log progress
      const stateChanged = session.state !== lastState
      if (stateChanged) {
        console.log(`[${state.iterations}] State: ${session.state} | Bash: ${outputs.length} | Failures: ${state.consecutiveFailures}`)
        lastState = session.state
        // Reset interval on state change
        currentInterval = config.minIntervalMs
      } else {
        process.stdout.write('.')
      }

      // Increase interval if no changes (exponential backoff)
      if (!stateChanged) {
        currentInterval = Math.min(config.maxIntervalMs, currentInterval * 1.5)
      }

      await writeStatus()

    } catch (err) {
      console.error(`\n[${state.iterations}] Error: ${err}`)
    }

    // Wait before next check
    await Bun.sleep(currentInterval)
  }

  state.message = `Max iterations (${config.maxIterations}) reached`
  notify(state.message)
  await writeStatus()
  return state
}

// CLI
async function main() {
  const args = process.argv.slice(2)

  if (args.length === 0 || args[0] === '--help') {
    console.log(`Smart Jules Session Monitor

Usage:
  bun run lib/session-monitor.ts <session-id> [options]

Options:
  --output <file>     Write status JSON to file (for external polling)
  --stuck <n>         Stuck threshold (default: 5 consecutive failures)
  --max-iter <n>      Max iterations before giving up (default: 100)

Examples:
  bun run lib/session-monitor.ts 123456
  bun run lib/session-monitor.ts 123456 --output /tmp/session-status.json
  bun run lib/session-monitor.ts 123456 --stuck 3 --max-iter 50

The monitor will:
1. Poll with exponential backoff (30s to 5min)
2. Detect stuck sessions (repeated compilation failures)
3. Exit and notify when completed or stuck
4. Write status to file for external tools to read
`)
    process.exit(0)
  }

  const sessionId = args[0]!
  let outputFile: string | undefined
  let stuckThreshold = DEFAULT_CONFIG.stuckThreshold
  let maxIterations = DEFAULT_CONFIG.maxIterations

  for (let i = 1; i < args.length; i++) {
    if (args[i] === '--output' && args[i + 1]) {
      outputFile = args[++i]
    } else if (args[i] === '--stuck' && args[i + 1]) {
      stuckThreshold = parseInt(args[++i]!, 10)
    } else if (args[i] === '--max-iter' && args[i + 1]) {
      maxIterations = parseInt(args[++i]!, 10)
    }
  }

  const config: MonitorConfig = {
    sessionId,
    outputFile,
    stuckThreshold,
    maxIterations,
    ...DEFAULT_CONFIG,
  }

  const result = await monitor(config)

  console.log('\n\n=== Final Status ===')
  console.log(JSON.stringify(result, null, 2))

  // Exit codes for scripting
  if (result.completed) {
    process.exit(0)
  } else if (result.stuckDetected) {
    process.exit(2)
  } else {
    process.exit(1)
  }
}

main().catch(err => {
  console.error('Fatal error:', err)
  process.exit(1)
})
