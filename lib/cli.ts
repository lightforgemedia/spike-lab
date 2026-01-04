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

Examples:
  bun run lib/cli.ts sessions
  bun run lib/cli.ts message 123456 "Try using the v2 API"
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

        const { activities } = await client.listActivities(sessionId, { pageSize: 5 })
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
