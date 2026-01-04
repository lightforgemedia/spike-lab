# Agentic Orchestrator (prototype)

A small, local-first agentic execution orchestrator:

- **Daemon** owns the embedded SurrealDB graph + schedules work.
- **Agents** pull jobs, run **exec blocks** (multiple commands), and write a reproducible **execution bundle** (stdout/stderr paths, status, timestamps, etc.).
- **Graph-native workflow**: stage dependencies are stored as graph edges (`stage_run ->requires-> stage_run`).

This repo is intentionally a **v0 that compiles and runs**, with clear seams for future hardening.

## What you get in this prototype

- Embedded SurrealDB (`surrealkv://…`) with schema + scheduler.
- At-least-once job delivery via leases (agent can crash; jobs re-queued after lease expiry).
- Exec-block runner that:
  - runs multiple commands
  - captures stdout/stderr to per-command files
  - emits a `manifest.json` describing every command (argv, status, timestamps, log paths)
- Pre-exec validation pass:
  - blocks shell entrypoints by default (`bash -c`, `sh -c`, `powershell`, etc.)
  - blocks/flags common destructive tools unless paths are safely relative to the working directory

## Requirements

- Rust **1.80.1+** (SurrealDB Rust SDK requirement; see SurrealDB docs)
- A POSIX-ish shell is helpful for the demo commands, but not strictly required.

## Quick start (local demo)

In one terminal:

```bash
cargo run -p orchestrator-daemon -- \
  --listen 127.0.0.1:3000 \
  --db-dir ./.orchestrator/db
```

In another terminal:

```bash
cargo run -p orchestrator-agent -- \
  --daemon http://127.0.0.1:3000 \
  --agent-id agent-1 \
  --runs-root ./.orchestrator/runs
```

Enqueue a demo workflow run:

```bash
curl -sS -X POST http://127.0.0.1:3000/v1/demo/enqueue \
  -H 'content-type: application/json' \
  -d '{ "project_path": ".", "description": "demo run" }' | jq
```

Watch the agent pick up the job, run commands, and write bundles under:

```
./.orchestrator/runs/<run_id>/<stage_id>/<exec_id>/
```

## Docs

Start at:

- `docs/SPEC.md`
- `docs/ARCHITECTURE.md`
- `docs/SCENARIOS.md`
- `docs/SECURITY.md`
- `docs/REJECTED.md`

## Non-goals (for this v0)

- Real multi-repo checkouts, `jj` workspace management, credential injection
- Remote artifact storage (bundle paths are local filesystem paths)
- Full DAG parallelism across multiple daemons

Those are deliberately deferred; see `docs/ROADMAP.md`.
