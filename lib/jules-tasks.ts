/**
 * Jules Task Management
 *
 * Create sessions from templates and archive results.
 * Zero external dependencies - uses JSON for archives, markdown for templates.
 */

import { JulesClient, type SourceContext } from './index'

export interface SessionArchive {
  sessionId: string
  url: string
  title: string
  source: string
  branch: string
  createdAt: string
  status: 'created' | 'planning' | 'executing' | 'completed' | 'failed'
  template?: string
  variables?: Record<string, string>
  prompt: string
  coverageTargets?: Record<string, number>
}

function getJulesDir(): string {
  return `${process.env.HOME}/PROJECTS/spike-lab/jules`
}

/**
 * Load a markdown template and extract the prompt section
 */
export async function loadTemplate(name: string): Promise<string> {
  const path = `${getJulesDir()}/templates/${name}.md`
  const content = await Bun.file(path).text()
  return content
}

/**
 * Render a template with ${VAR} substitution
 */
export function renderTemplate(template: string, variables: Record<string, string>): string {
  let result = template
  for (const [key, value] of Object.entries(variables)) {
    result = result.replace(new RegExp(`\\$\\{${key}\\}`, 'g'), value)
  }
  return result
}

/**
 * Archive a session as JSON
 */
export async function archiveSession(archive: SessionArchive): Promise<void> {
  const path = `${getJulesDir()}/sessions/${archive.sessionId}.json`
  await Bun.write(path, JSON.stringify(archive, null, 2))
}

/**
 * Load a session archive
 */
export async function loadSession(sessionId: string): Promise<SessionArchive | null> {
  const path = `${getJulesDir()}/sessions/${sessionId}.json`
  try {
    const content = await Bun.file(path).text()
    return JSON.parse(content) as SessionArchive
  } catch {
    return null
  }
}

/**
 * Update session status
 */
export async function updateSessionStatus(
  sessionId: string,
  status: SessionArchive['status']
): Promise<void> {
  const archive = await loadSession(sessionId)
  if (archive) {
    archive.status = status
    await archiveSession(archive)
  }
}

/**
 * Create a Jules session from a template
 */
export async function createSessionFromTemplate(options: {
  template: string
  title: string
  source: string
  branch?: string
  variables: Record<string, string>
  coverageTargets?: Record<string, number>
  apiKey?: string
}): Promise<{ sessionId: string; url: string }> {
  const { template: templateName, title, source, branch, variables, coverageTargets, apiKey } = options

  // Load and render template
  const template = await loadTemplate(templateName)
  const prompt = renderTemplate(template, variables)

  // Create Jules client
  const clientConfig: { apiKey?: string } = {}
  if (apiKey) clientConfig.apiKey = apiKey
  const client = new JulesClient(clientConfig)

  // Build source context
  const sourceContext: SourceContext = { source }
  if (branch) {
    sourceContext.githubRepoContext = { startingBranch: branch }
  }

  // Create session
  const session = await client.createSession({
    prompt,
    sourceContext,
    title,
  })

  // Archive the session
  const archive: SessionArchive = {
    sessionId: session.id,
    url: session.url,
    title,
    source,
    branch: branch ?? 'main',
    createdAt: new Date().toISOString(),
    status: 'created',
    template: templateName,
    variables,
    prompt,
  }
  if (coverageTargets) {
    archive.coverageTargets = coverageTargets
  }

  await archiveSession(archive)

  return { sessionId: session.id, url: session.url }
}

/**
 * List archived sessions
 */
export async function listArchivedSessions(): Promise<SessionArchive[]> {
  const glob = new Bun.Glob('*.json')
  const sessions: SessionArchive[] = []

  for await (const file of glob.scan(`${getJulesDir()}/sessions`)) {
    try {
      const content = await Bun.file(`${getJulesDir()}/sessions/${file}`).text()
      sessions.push(JSON.parse(content) as SessionArchive)
    } catch {
      // Skip invalid files
    }
  }

  return sessions.sort((a, b) => b.createdAt.localeCompare(a.createdAt))
}
