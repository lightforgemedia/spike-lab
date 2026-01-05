/**
 * Project Profiler - Gathers context before Jules sessions
 *
 * Pit of Success: Pre-compute everything Jules needs so it can't go wrong
 */

import { $ } from 'bun'

export interface ProjectProfile {
  path: string
  language: 'rust' | 'typescript' | 'go' | 'unknown'
  buildTool: string
  crates: string[]
  existingTests: string[]
  publicTypes: string[]
  newtypes: string[]
  testPattern: string | null
  warnings: string[]
  spikeContext: string | null  // Content from jules.md if present
}

export async function profileRustProject(projectPath: string): Promise<ProjectProfile> {
  const profile: ProjectProfile = {
    path: projectPath,
    language: 'rust',
    buildTool: 'cargo',
    crates: [],
    existingTests: [],
    publicTypes: [],
    newtypes: [],
    testPattern: null,
    warnings: [],
    spikeContext: null,
  }

  // Check for spike-specific context (jules.md in parent or project dir)
  const julesLocations = [
    `${projectPath}/jules.md`,
    `${projectPath}/../jules.md`,  // Parent dir (spike root)
  ]
  for (const loc of julesLocations) {
    try {
      const content = await Bun.file(loc).text()
      profile.spikeContext = content
      break
    } catch {
      // File doesn't exist, try next
    }
  }

  // Find crates
  try {
    const cratesDir = `${projectPath}/crates`
    const result = await $`ls ${cratesDir} 2>/dev/null`.text()
    profile.crates = result.trim().split('\n').filter(Boolean)
  } catch {
    profile.warnings.push('No crates/ directory found')
  }

  // Find existing tests
  try {
    const result = await $`find ${projectPath} -name "*.rs" -path "*/tests/*" 2>/dev/null`.text()
    profile.existingTests = result.trim().split('\n').filter(Boolean).slice(0, 10)
  } catch {
    // No tests found
  }

  // Find public types
  try {
    const result = await $`grep -r "^pub struct\\|^pub enum\\|^pub type" ${projectPath}/crates/*/src/lib.rs 2>/dev/null`.text()
    profile.publicTypes = result.trim().split('\n').filter(Boolean).slice(0, 20)
  } catch {
    // No public types found
  }

  // Find newtypes (single-field tuple structs)
  try {
    const result = await $`grep -r "^pub struct.*(" ${projectPath}/crates/*/src/ 2>/dev/null`.text()
    const newtypePattern = /pub struct (\w+)\([^)]+\)/g
    const matches = result.matchAll(newtypePattern)
    profile.newtypes = [...matches].map(m => m[1])
  } catch {
    // No newtypes found
  }

  // Extract test pattern from existing tests
  if (profile.existingTests.length > 0) {
    try {
      const testFile = profile.existingTests[0]
      const content = await Bun.file(testFile).text()
      // Extract first 30 lines as pattern example
      profile.testPattern = content.split('\n').slice(0, 30).join('\n')
    } catch {
      // Couldn't read test file
    }
  }

  // Check for common pitfalls
  try {
    const cargoToml = await Bun.file(`${projectPath}/Cargo.toml`).text()
    if (cargoToml.includes('surrealdb')) {
      profile.warnings.push('SurrealDB detected: enums may need #[serde(tag = "type")]')
    }
    if (cargoToml.includes('tokio')) {
      profile.warnings.push('Tokio detected: async tests need #[tokio::test]')
    }
  } catch {
    // No Cargo.toml found
  }

  return profile
}

export function generateTestPromptContext(profile: ProjectProfile): string {
  const sections: string[] = []

  // If spike-specific context exists, use it as primary source
  if (profile.spikeContext) {
    sections.push(`## Spike-Specific Instructions

The following instructions are specific to this project. **Follow these exactly.**

${profile.spikeContext}`)
  }

  // Current state (auto-detected, supplements spike context)
  sections.push(`## Auto-Detected Context
- Crates: ${profile.crates.join(', ') || 'none found'}
- Existing test files: ${profile.existingTests.length}
- Newtypes detected: ${profile.newtypes.join(', ') || 'none'}`)

  // Warnings (only if no spike context, since spike context should cover these)
  if (!profile.spikeContext && profile.warnings.length > 0) {
    sections.push(`## Warnings
${profile.warnings.map(w => `- ${w}`).join('\n')}`)
  }

  // Existing test pattern (only if no spike context provides one)
  if (!profile.spikeContext && profile.testPattern) {
    sections.push(`## Existing Test Pattern (FOLLOW THIS)
\`\`\`rust
${profile.testPattern}
\`\`\``)
  }

  return sections.join('\n\n')
}

export function generateTaskList(profile: ProjectProfile, coverageTargets: Record<string, number>): string {
  const tasks: string[] = []

  for (const crate of profile.crates) {
    const target = coverageTargets[crate] || 70
    tasks.push(`### ${crate} (target: ${target}% coverage)

**File to create**: \`crates/${crate}/tests/${crate}_tests.rs\`

Steps:
1. \`mkdir -p crates/${crate}/tests/\`
2. Create test file with imports from \`${crate.replace(/-/g, '_')}\` crate
3. Add tests for public types
4. Run \`cargo test -p ${crate}\` - must pass before continuing
`)
  }

  return tasks.join('\n')
}

// CLI usage
if (import.meta.main) {
  const projectPath = process.argv[2]
  if (!projectPath) {
    console.error('Usage: bun run lib/project-profiler.ts <project-path>')
    process.exit(1)
  }

  const profile = await profileRustProject(projectPath)
  console.log('=== Project Profile ===')
  console.log(JSON.stringify(profile, null, 2))
  console.log('\n=== Generated Context ===')
  console.log(generateTestPromptContext(profile))
}
