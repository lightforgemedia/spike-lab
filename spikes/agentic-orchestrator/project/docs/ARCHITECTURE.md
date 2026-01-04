# Architecture

## Components

**Daemon**
- Owns the embedded SurrealDB (SurrealKV).
- Stores projects, runs, stages, jobs, leases, and agent heartbeats.
- Schedules runnable stages into jobs.
- Hands jobs to agents via a pull-based HTTP API.
- Optionally runs retention garbage collection (disabled by default).

**Agent**
- Polls the daemon for work (`/v1/agent/claim`).
- Executes the job's exec block (Local or Slurm).
- Streams stdout/stderr to per-command log files in a run bundle directory.
- Sends periodic heartbeats to extend a job lease.
- Posts completion and output revision (`/v1/agent/complete`).

**Core**
- Shared API models.
- Command/path safety guardrails.

## Execution flow

1. A run is enqueued (demo endpoint creates a run with two stages and exec blocks).
2. The scheduler finds stages whose dependencies are satisfied and creates a queued job for each.
3. An agent claims a queued job:
   - daemon issues a lease token and expiry timestamp
   - daemon prepares a workspace directory (jj workspace if available)
4. Agent executes commands:
   - each command writes stdout/stderr to files
   - each command produces a JSON meta record with timestamps and exit status
5. Agent reports completion + the output repo revision id.
6. Daemon marks stage complete and wires downstream stages to the output revision.

## Revision-aware multi-agent work (jj)

- The daemon captures a base revision when the run is enqueued.
- A stage's job includes an `input_revision` (base revision for first stage; otherwise the upstream stage output revision).
- The daemon creates a new workspace for the job using:

  `jj --repository <project_root> workspace add --revision <input_rev> <workspace_path>`

This creates a job-specific working copy commit parented by the input revision,
which is safe for multiple concurrent agents.
