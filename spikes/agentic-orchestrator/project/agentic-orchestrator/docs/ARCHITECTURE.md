# Architecture

## Components

- **Daemon** (`orchestrator-daemon`)
  - Owns the embedded SurrealDB datastore
  - Stores workflow graphs and run graphs
  - Schedules runnable stages into lease-based jobs
  - Exposes a small HTTP API to enqueue runs and for agents to claim/complete jobs

- **Agent** (`orchestrator-agent`)
  - Polls the daemon for a job lease
  - Validates the exec spec (defense-in-depth)
  - Executes an **exec block** (a list of commands)
  - Writes an **execution bundle** to disk and reports results to the daemon

- **Core** (`orchestrator-core`)
  - Types, validation, and scheduling helpers
  - Shared API types used by both daemon and agent

## Data model overview (graph)

SurrealDB is a document-graph DB, so we store *runtime* dependencies as edges.

Nodes:
- `run`
- `stage_run`
- `job`
- `artifact` (execution bundle pointer)

Edges:
- `stage_run ->requires-> stage_run`

Semantics:
- An edge `B ->requires-> A` means **B depends on A**.
- A `stage_run` is runnable when **all required stages are succeeded**.

## Execution bundle layout

Agents write immutable, append-only bundles:

```
.orchestrator/runs/
  <run_id>/
    <stage_id>/
      <exec_id>/
        manifest.json
        cmd-000.stdout.log
        cmd-000.stderr.log
        cmd-001.stdout.log
        cmd-001.stderr.log
        ...
```

The daemon stores pointers to these bundles in the database (prototype assumes
daemon + agents share a filesystem).

## Intent clarity

This system makes intent explicit at three levels:

1. **Workflow intent**: stage graph and stage kinds (what we want done).
2. **Execution intent**: exec-block spec is structured (argv, cwd, env), not a shell string.
3. **Safety intent**: validation gates (shell blocked unless explicit allow) plus path constraints.

## Phased execution flow

A run proceeds through phases:

1. Run created (workflow graph materialized as `stage_run` nodes + edges)
2. Scheduler enqueues runnable stages as `job` records
3. Agents claim jobs via lease, execute, and write execution bundle
4. Agent reports completion; daemon commits results and schedules downstream stages
