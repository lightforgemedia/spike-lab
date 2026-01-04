# spike-lab

Research spike management with [Jules](https://jules.google.com) integration.

## Setup

```bash
# Add to PATH
export PATH="$PATH:$HOME/PROJECTS/spike-lab/bin"

# Set up Jules API key
spike api-setup
```

## Quick Start

```bash
# Create a new spike
spike new graphql-perf "Investigate GraphQL query performance"

# Send task to Jules
spike jules graphql-perf "Profile and optimize N+1 queries"

# List all spikes
spike list
```

## Jules SDK (TypeScript)

Full-featured SDK for programmatic Jules interaction:

```typescript
import { JulesClient } from './lib'

const client = new JulesClient()

// Monitor and extract patches
const { session, patch } = await client.waitAndExtractPatch(sessionId)
if (patch) {
  await Bun.write('fix.patch', patch.patch)
  // git apply fix.patch
}
```

### CLI Commands

```bash
# Session management
bun run lib/cli.ts sessions              # List sessions
bun run lib/cli.ts status <id>           # Get session status
bun run lib/cli.ts message <id> "help"   # Send message
bun run lib/cli.ts approve <id>          # Approve plan

# Patch extraction (main workflow)
bun run lib/cli.ts apply <id> [dir]      # Full workflow: wait, extract, apply, verify
bun run lib/cli.ts patch <id>            # Extract latest patch
bun run lib/cli.ts patches <id>          # Extract all patches

# Debugging
bun run lib/cli.ts plan <id>             # View plan steps
bun run lib/cli.ts bash <id>             # View bash commands run
```

## Project Structure

```
spike-lab/
├── bin/spike              # Bash CLI
├── lib/                   # TypeScript SDK
│   ├── client.ts          # JulesClient
│   ├── types.ts           # API types
│   ├── errors.ts          # Error classes
│   ├── cli.ts             # CLI wrapper
│   └── index.ts           # Exports
├── spikes/                # Research spikes
│   ├── spl/               # SPL pipeline spike
│   └── agentic-orchestrator/  # Orchestrator spike
```

## Jules Workflow

Recommended workflow for using Jules effectively:

1. **Create focused task** - Specific file + error codes work best
2. **Monitor session** - `bun run lib/cli.ts status <id>`
3. **Extract and apply** - `bun run lib/cli.ts apply <id> ./project`
4. **Verify locally** - Run tests/checks before committing

Jules generates patches but doesn't create PRs reliably. The SDK extracts patches for local application.

## Current Spikes

| Spike | Status | Description |
|-------|--------|-------------|
| spl | Compiles | Software pipeline with gates and evidence |
| agentic-orchestrator | Compiles | Task orchestrator with SurrealDB |
