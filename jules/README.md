# Jules Task Management

Version-controlled prompts and session archives for Jules AI coding tasks.

## Structure

```
spike-lab/
├── jules/                      # General Jules infrastructure
│   ├── templates/              # General prompt templates
│   │   ├── add-tests-general.md
│   │   └── project-profile.md
│   ├── sessions/               # Archived session records
│   │   └── {session-id}.json
│   ├── LESSONS_LEARNED.md      # Issue log and pit-of-success methodology
│   └── README.md
│
├── lib/                        # CLI and profiler
│   ├── cli.ts                  # Jules CLI (add-tests, status, etc.)
│   └── project-profiler.ts     # Auto-detect project context
│
└── spikes/
    └── {spike-name}/
        └── jules.md            # Spike-specific instructions
```

## Two-Layer Approach

### 1. General Instructions (`jules/templates/`)

Language-agnostic guidelines that apply to all projects:
- Failure protocol (STOP after 3 failures)
- Test structure (integration vs unit)
- Discovery phase before coding
- Deliverables checklist

### 2. Spike-Specific Instructions (`spikes/{name}/jules.md`)

Project-specific details that the profiler auto-includes:
- Crate/package structure
- Known pitfalls (SurrealDB, newtypes, etc.)
- Existing test patterns to follow
- What NOT to do
- Coverage targets

## CLI Commands

```bash
# Create test session with auto-profiling (uses jules.md if present)
bun run lib/cli.ts add-tests ./spikes/my-spike/project --coverage core:80

# Check session status
bun run lib/cli.ts status <session-id>

# Send guidance to stuck session
bun run lib/cli.ts message <session-id> "Try this approach..."

# Extract and apply patch
bun run lib/cli.ts patch <session-id> > fix.patch && git apply fix.patch
```

## Creating Spike-Specific Context

When creating a new spike, add a `jules.md` file:

```markdown
# Jules Context: {spike-name}

## Project Structure
[Describe crate/package layout]

## Known Pitfalls
[Document things that will trip up Jules]

## Existing Test Pattern
[Paste working test example]

## What NOT To Do
[Explicit anti-patterns]

## Commands
[Build, test, coverage commands]

## Coverage Targets
[Target percentages per module]
```

## Profiler Auto-Detection

The `add-tests` command runs a profiler that:

1. **Finds** `jules.md` in spike directory
2. **Detects** crates/packages automatically
3. **Extracts** existing test patterns
4. **Warns** about known pitfalls (SurrealDB, Tokio, etc.)
5. **Generates** structured prompt combining general + spike-specific

## Session Lifecycle

1. **Create**: `bun run lib/cli.ts add-tests <path>`
2. **Monitor**: `bun run lib/cli.ts status <id>`
3. **Guide**: `bun run lib/cli.ts message <id> "..."` (if stuck)
4. **Apply**: `bun run lib/cli.ts apply <id>` (when complete)
5. **Archive**: Auto-saved to `jules/sessions/{id}.json`

## Coverage Measurement

```bash
# Install
cargo install cargo-tarpaulin

# Run with HTML report
cargo tarpaulin --out Html --output-dir coverage/
```

### Coverage Targets

| Type | Target |
|------|--------|
| Core business logic | 80%+ |
| Service layers | 70%+ |
| CLI/runners | 60%+ |
