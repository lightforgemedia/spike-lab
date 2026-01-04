#!/usr/bin/env bun
/**
 * Jules API CLI - For agent/programmatic interaction
 *
 * Usage:
 *   bun run lib/cli.ts sessions
 *   bun run lib/cli.ts status <session-id>
 *   bun run lib/cli.ts message <session-id> "your message"
 *   bun run lib/cli.ts approve <session-id>
 *   bun run lib/cli.ts wait <session-id>
 */

import { JulesClient, JulesError } from './index'

// Load API key from ~/.spike-lab
async function loadApiKey(): Promise<string> {
  const configPath = `${process.env.HOME}/.spike-lab`
  try {
    const configFile = Bun.file(configPath)
    const configText = await configFile.text()
    const match = configText.match(/JULES_API_KEY="([^"]+)"/)
    if (match?.[1]) return match[1]
  } catch {
    // Fall through to env var
  }

  const envKey = process.env['JULES_API_KEY']
  if (envKey) return envKey

  console.error('Error: No API key found')
  console.error('Run: spike api-setup')
  process.exit(1)
}

async function main() {
  const [command, ...args] = process.argv.slice(2)

  if (!command || command === 'help' || command === '--help') {
    console.log(`Jules API CLI

Commands:
  sessions                    List all sessions
  status <session-id>         Get session status and recent activities
  message <session-id> <msg>  Send message to session
  approve <session-id>        Approve session plan
  wait <session-id>           Wait for session to complete
  patch <session-id>          Extract latest patch from session
  patches <session-id>        Extract all patches from session
  plan <session-id>           Get current plan steps
  bash <session-id>           List bash command outputs

Examples:
  bun run lib/cli.ts sessions
  bun run lib/cli.ts message 123456 "Try using the v2 API"
  bun run lib/cli.ts patch 123456 > fix.patch && git apply fix.patch
`)
    process.exit(0)
  }

  const apiKey = await loadApiKey()
  const client = new JulesClient({ apiKey })

  try {
    switch (command) {
      case 'sessions': {
        const { sessions } = await client.listSessions({ pageSize: 20 })
        if (sessions.length === 0) {
          console.log('No sessions found')
        } else {
          console.log('Sessions:')
          for (const s of sessions) {
            console.log(`  ${s.id} [${s.state}] ${s.title ?? s.prompt.slice(0, 60)}`)
          }
        }
        break
      }

      case 'status': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: status <session-id>')
          process.exit(1)
        }

        const session = await client.getSession(sessionId)
        console.log(`Session: ${session.id}`)
        console.log(`State: ${session.state}`)
        console.log(`Title: ${session.title}`)
        console.log(`URL: ${session.url}`)

        const response = await client.listActivities(sessionId, { pageSize: 5 })
        const activities = response.activities ?? []
        if (activities.length > 0) {
          console.log('\nRecent activities:')
          for (const a of activities.slice(-5)) {
            const content =
              (a as any).userMessaged?.userMessage ??
              (a as any).agentMessaged?.agentMessage ??
              a.content ??
              a.summary ??
              '(no content)'
            console.log(`  [${a.originator}] ${content.slice(0, 100)}`)
          }
        }
        break
      }

      case 'message': {
        const sessionId = args[0]
        const message = args.slice(1).join(' ')
        if (!sessionId || !message) {
          console.error('Usage: message <session-id> <message>')
          process.exit(1)
        }

        await client.sendMessage(sessionId, message)
        console.log(`Message sent to session ${sessionId}`)
        console.log(`Message: ${message}`)
        break
      }

      case 'approve': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: approve <session-id>')
          process.exit(1)
        }

        await client.approvePlan(sessionId)
        console.log(`Plan approved for session ${sessionId}`)
        break
      }

      case 'wait': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: wait <session-id>')
          process.exit(1)
        }

        console.log(`Waiting for session ${sessionId} to complete...`)
        const session = await client.waitForCompletion(sessionId, {
          pollInterval: 5000,
          timeout: 300000,
        })
        console.log(`Session completed with state: ${session.state}`)
        break
      }

      case 'patch': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: patch <session-id>')
          process.exit(1)
        }

        const patch = await client.getLatestPatch(sessionId)
        if (!patch) {
          console.error('No patches found in session')
          process.exit(1)
        }

        // Output just the patch (for piping to git apply)
        console.log(patch.patch)
        break
      }

      case 'patches': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: patches <session-id>')
          process.exit(1)
        }

        const patches = await client.extractPatches(sessionId)
        if (patches.length === 0) {
          console.error('No patches found in session')
          process.exit(1)
        }

        console.error(`Found ${patches.length} patch(es):`)
        for (let i = 0; i < patches.length; i++) {
          const p = patches[i]!
          console.error(`\n--- Patch ${i + 1} (${p.activityId}) ---`)
          if (p.suggestedCommitMessage) {
            console.error(`Commit message: ${p.suggestedCommitMessage.split('\n')[0]}`)
          }
          console.log(p.patch)
        }
        break
      }

      case 'plan': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: plan <session-id>')
          process.exit(1)
        }

        const plan = await client.getPlan(sessionId)
        if (!plan) {
          console.log('No plan found for session')
        } else {
          console.log(`Plan (${plan.id}):`)
          for (const step of plan.steps) {
            const idx = step.index ?? 0
            console.log(`  ${idx + 1}. ${step.title}`)
          }
        }
        break
      }

      case 'bash': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: bash <session-id>')
          process.exit(1)
        }

        const outputs = await client.extractBashOutputs(sessionId)
        if (outputs.length === 0) {
          console.log('No bash commands found in session')
        } else {
          console.log(`Found ${outputs.length} command(s):`)
          for (const out of outputs) {
            const status = out.exitCode === 0 ? '✓' : out.exitCode === null ? '?' : `✗(${out.exitCode})`
            const cmd = out.command.length > 80 ? out.command.slice(0, 77) + '...' : out.command
            console.log(`  ${status} ${cmd}`)
          }
        }
        break
      }

      default:
        console.error(`Unknown command: ${command}`)
        console.error('Run with --help for usage')
        process.exit(1)
    }
  } catch (err) {
    if (err instanceof JulesError) {
      console.error(`API Error [${err.code}]: ${err.message}`)
    } else {
      console.error('Error:', err)
    }
    process.exit(1)
  }
}

main()
