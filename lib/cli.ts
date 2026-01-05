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
import { runSplWithJulesDelegate, type SplProject } from './spl-jules'

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
  apply <session-id> [dir]    Wait, extract, apply patch, verify (full workflow)
  patch <session-id>          Extract latest patch from session
  patches <session-id>        Extract all patches from session
  plan <session-id>           Get current plan steps
  bash <session-id>           List bash command outputs

Test Generation (Pit of Success):
  add-tests <project-path>    Profile project and create test session
                              Options: --source, --branch, --coverage core:80,daemon:70

Monitoring:
  monitor <session-id>        Start smart monitor (exits on complete/stuck)
                              Options: --output <file>, --stuck <n>
  check <session-id>          Quick status check (single poll)

SPL Integration:
  spl-delegate <spec.yaml>    Run spec pack through Jules delegate gate
                              Options: --source, --branch, --dir

Examples:
  bun run lib/cli.ts sessions
  bun run lib/cli.ts message 123456 "Try using the v2 API"
  bun run lib/cli.ts patch 123456 > fix.patch && git apply fix.patch
  bun run lib/cli.ts add-tests ./spikes/my-project/project --coverage core:80
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

      case 'apply': {
        const sessionId = args[0]
        const workDir = args[1] || process.cwd()
        if (!sessionId) {
          console.error('Usage: apply <session-id> [directory]')
          process.exit(1)
        }

        // Step 1: Wait for session and extract patch
        console.log(`⏳ Waiting for session ${sessionId}...`)
        let lastState = ''
        const { session, patch } = await client.waitAndExtractPatch(sessionId, {
          pollInterval: 5000,
          timeout: 600000,
          onProgress: (state) => {
            if (state !== lastState) {
              console.log(`   State: ${state}`)
              lastState = state
            }
          },
        })

        if (session.state !== 'COMPLETED') {
          console.error(`❌ Session ended with state: ${session.state}`)
          process.exit(1)
        }

        if (!patch) {
          console.error('❌ No patch found in session')
          process.exit(1)
        }

        console.log(`✓ Session completed, patch extracted (${patch.patch.split('\n').length} lines)`)

        // Step 2: Write patch to temp file
        const patchFile = `/tmp/jules-${sessionId}.patch`
        await Bun.write(patchFile, patch.patch)
        console.log(`✓ Patch written to ${patchFile}`)

        // Step 3: Apply patch
        console.log(`⏳ Applying patch in ${workDir}...`)
        const applyProc = Bun.spawn(['git', 'apply', '--check', patchFile], {
          cwd: workDir,
          stdout: 'pipe',
          stderr: 'pipe',
        })
        const applyResult = await applyProc.exited

        if (applyResult !== 0) {
          const stderr = await new Response(applyProc.stderr).text()
          console.error(`❌ Patch check failed:\n${stderr}`)
          console.error(`Patch saved at: ${patchFile}`)
          process.exit(1)
        }

        // Actually apply
        const realApply = Bun.spawn(['git', 'apply', patchFile], {
          cwd: workDir,
          stdout: 'pipe',
          stderr: 'pipe',
        })
        await realApply.exited
        console.log('✓ Patch applied')

        // Step 4: Verify (try cargo check if Cargo.toml exists)
        const cargoToml = `${workDir}/Cargo.toml`
        const hasCargoToml = await Bun.file(cargoToml).exists()

        if (hasCargoToml) {
          console.log('⏳ Running cargo check...')
          const checkProc = Bun.spawn(['cargo', 'check'], {
            cwd: workDir,
            stdout: 'pipe',
            stderr: 'pipe',
          })
          const checkResult = await checkProc.exited

          if (checkResult !== 0) {
            const stderr = await new Response(checkProc.stderr).text()
            console.error(`⚠️  cargo check failed:\n${stderr.slice(-500)}`)
          } else {
            console.log('✓ cargo check passed')
          }
        }

        // Summary
        console.log(`\n✅ Done! Patch from session ${sessionId} applied.`)
        if (patch.suggestedCommitMessage) {
          console.log(`\nSuggested commit message:\n${patch.suggestedCommitMessage}`)
        }
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

      case 'monitor': {
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: monitor <session-id> [--output file] [--stuck n]')
          process.exit(1)
        }

        // Spawn the monitor as a separate process
        const monitorArgs = ['run', 'lib/session-monitor.ts', sessionId, ...args.slice(1)]
        const proc = Bun.spawn(['bun', ...monitorArgs], {
          stdout: 'inherit',
          stderr: 'inherit',
        })
        const exitCode = await proc.exited
        process.exit(exitCode)
      }

      case 'check': {
        // Quick single-poll status check with stuck detection
        const sessionId = args[0]
        if (!sessionId) {
          console.error('Usage: check <session-id>')
          process.exit(1)
        }

        const session = await client.getSession(sessionId)
        const outputs = await client.extractBashOutputs(sessionId)

        // Analyze for stuck pattern
        const recentOutputs = outputs.slice(-10)
        const failCount = recentOutputs.filter(o => o.exitCode === 101).length
        const isStuck = failCount >= 5

        console.log(`Session: ${session.id}`)
        console.log(`State: ${session.state}`)
        console.log(`Bash commands: ${outputs.length}`)
        console.log(`Recent failures: ${failCount}/10`)
        console.log(`Status: ${session.state === 'COMPLETED' ? '✅ DONE' : isStuck ? '🚨 STUCK' : '⏳ RUNNING'}`)

        if (isStuck) {
          console.log(`\nRecommendation: Send guidance with 'message' command`)
        }

        // Write to known location for external tools
        const statusFile = `/tmp/jules-${sessionId}.json`
        await Bun.write(statusFile, JSON.stringify({
          sessionId,
          state: session.state,
          bashCommands: outputs.length,
          recentFailures: failCount,
          isStuck,
          isCompleted: session.state === 'COMPLETED',
          checkedAt: new Date().toISOString(),
        }, null, 2))
        console.log(`\nStatus written to: ${statusFile}`)
        break
      }

      case 'add-tests': {
        // Usage: add-tests <project-path> [--source X] [--branch Y]
        const { profileRustProject, generateTestPromptContext, generateTaskList } = await import('./project-profiler')

        let projectPath = ''
        let source = ''
        let branch = 'main'
        const coverageTargets: Record<string, number> = {}

        for (let i = 0; i < args.length; i++) {
          const arg = args[i] ?? ''
          if (arg === '--source' && args[i + 1]) {
            source = args[++i] ?? ''
          } else if (arg === '--branch' && args[i + 1]) {
            branch = args[++i] ?? ''
          } else if (arg === '--coverage' && args[i + 1]) {
            // Format: --coverage core:80,daemon:70
            const pairs = (args[++i] ?? '').split(',')
            for (const pair of pairs) {
              const [crate, pct] = pair.split(':')
              if (crate && pct) {
                coverageTargets[crate] = parseInt(pct, 10)
              }
            }
          } else if (!arg.startsWith('--')) {
            projectPath = arg
          }
        }

        if (!projectPath) {
          console.error('Usage: add-tests <project-path> [--source owner/repo] [--branch main] [--coverage core:80,daemon:70]')
          process.exit(1)
        }

        // Profile the project
        console.log('📊 Profiling project...')
        const profile = await profileRustProject(projectPath)

        // Set default coverage targets if not specified
        for (const crate of profile.crates) {
          if (!coverageTargets[crate]) {
            coverageTargets[crate] = crate === 'core' ? 80 : 70
          }
        }

        // Detect source from git if not specified
        if (!source) {
          const { $ } = await import('bun')
          const remote = await $`git remote get-url origin`.quiet().nothrow()
          if (remote.exitCode === 0) {
            const url = remote.stdout.toString().trim()
            const match = url.match(/github\.com[:/]([^/]+\/[^/.]+)/)
            if (match?.[1]) {
              source = `sources/github/${match[1].replace('.git', '')}`
            }
          }
        }

        if (!source) {
          console.error('Error: Could not detect GitHub source. Use --source owner/repo')
          process.exit(1)
        }
        if (!source.startsWith('sources/')) {
          source = `sources/github/${source}`
        }

        // Generate the prompt
        const contextSection = generateTestPromptContext(profile)
        const tasksSection = generateTaskList(profile, coverageTargets)

        const prompt = `# Add Comprehensive Tests

${contextSection}

## Tasks
${tasksSection}

## CRITICAL: Failure Protocol

**STOP after 3 consecutive test failures.** Do not continue making changes.
Instead:
1. Report the exact error message
2. Explain what you were trying to do
3. Wait for guidance

## Test Structure (FOLLOW EXACTLY)

Create integration tests in \`crates/{crate}/tests/\` directory:
- Do NOT add \`mod tests;\` to lib.rs for integration tests
- Import crate as external: \`use crate_name::*;\`
- Run \`cargo test -p {crate}\` after each change

## Deliverables

- [ ] Each crate has tests in crates/{crate}/tests/
- [ ] All tests pass: cargo test exits 0
- [ ] tarpaulin.toml created for coverage

Do NOT modify production code.`

        console.log('\n📋 Generated prompt:')
        console.log('─'.repeat(60))
        console.log(prompt.slice(0, 500) + '...')
        console.log('─'.repeat(60))

        console.log('\n🚀 Creating Jules session...')
        const session = await client.createSession({
          prompt,
          title: `[Tests] Add tests to ${profile.crates.join(', ')}`,
          sourceContext: {
            source,
            githubRepoContext: { startingBranch: branch },
          },
        })

        console.log(`\n✅ Session created:`)
        console.log(`   ID: ${session.id}`)
        console.log(`   URL: ${session.url}`)
        console.log(`   State: ${session.state}`)

        // Archive the session
        const archivePath = `${process.cwd()}/jules/sessions/${session.id}.json`
        const archive = {
          sessionId: session.id,
          url: session.url,
          title: session.title,
          source,
          branch,
          createdAt: new Date().toISOString(),
          status: 'created',
          template: 'add-tests-v2',
          profile: {
            crates: profile.crates,
            existingTests: profile.existingTests.length,
            warnings: profile.warnings,
          },
          coverageTargets,
        }

        await Bun.write(archivePath, JSON.stringify(archive, null, 2))
        console.log(`   Archive: ${archivePath}`)
        break
      }

      case 'spl-delegate': {
        // Parse args: spl-delegate <spec.yaml> [--source X] [--branch Y] [--dir Z]
        let specPath = ''
        let source = ''
        let branch = ''
        let workDir = process.cwd()

        for (let i = 0; i < args.length; i++) {
          const arg = args[i] ?? ''
          if (arg === '--source' && args[i + 1]) {
            source = args[++i] ?? ''
          } else if (arg === '--branch' && args[i + 1]) {
            branch = args[++i] ?? ''
          } else if (arg === '--dir' && args[i + 1]) {
            workDir = args[++i] ?? process.cwd()
          } else if (!arg.startsWith('--')) {
            specPath = arg
          }
        }

        if (!specPath) {
          console.error('Usage: spl-delegate <spec.yaml> [--source owner/repo] [--branch main] [--dir .]')
          console.error('\nExample:')
          console.error('  bun run lib/cli.ts spl-delegate ./spec.yaml --source owner/repo --dir ./project')
          process.exit(1)
        }

        // Try to detect source from git remote if not specified
        if (!source) {
          const { $ } = await import('bun')
          const remote = await $`git remote get-url origin`.cwd(workDir).quiet().nothrow()
          if (remote.exitCode === 0) {
            const url = remote.stdout.toString().trim()
            // Parse github.com:owner/repo or https://github.com/owner/repo
            const match = url.match(/github\.com[:/]([^/]+\/[^/.]+)/)
            if (match?.[1]) {
              source = `sources/github/${match[1].replace('.git', '')}`
              console.log(`Detected source: ${source}`)
            }
          }
        }

        if (!source) {
          console.error('Error: Could not detect GitHub source. Use --source owner/repo')
          process.exit(1)
        }

        // Normalize source format
        if (!source.startsWith('sources/')) {
          source = `sources/github/${source}`
        }

        const project: SplProject = {
          repoRoot: workDir,
          source,
        }
        if (branch) {
          project.branch = branch
        }

        console.log(`\n🔧 SPL + Jules Delegate`)
        console.log(`   Spec: ${specPath}`)
        console.log(`   Source: ${source}`)
        console.log(`   Dir: ${workDir}\n`)

        const result = await runSplWithJulesDelegate(project, specPath, {
          apiKey,
          timeout: 600000,
          onProgress: (state, msg) => {
            console.log(`   [${state}] ${msg ?? ''}`)
          },
        })

        console.log(`\n📋 Results:`)
        console.log(`   Session: ${result.delegateResult.sessionId}`)
        console.log(`   URL: ${result.delegateResult.sessionUrl}`)

        if (result.delegateResult.applied) {
          console.log(`   ✓ Patch applied successfully`)
        } else if (result.delegateResult.patchFile) {
          console.log(`   ⚠ Patch extracted but not applied: ${result.delegateResult.patchFile}`)
        }

        if (result.delegateResult.error) {
          console.error(`   ✗ Error: ${result.delegateResult.error}`)
        }

        if (result.nextSteps.length > 0) {
          console.log(`\n📌 Next steps:`)
          for (const step of result.nextSteps) {
            console.log(`   • ${step}`)
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
