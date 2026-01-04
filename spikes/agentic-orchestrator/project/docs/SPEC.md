# Specification (v0)

This is the buildable v0 spec reflected in code.

## Terminology

- **WorkflowSpec**: declarative stage graph (definitions + edges).
- **Run**: one instance of executing a workflow for a project.
- **StageRun**: runtime node corresponding to a stage definition within a run.
- **Job**: a lease-based execution unit for an agent. A StageRun can have multiple Jobs over time (retries).

## Invariants

- A StageRun is **Succeeded** at most once.
- A Job lease is **owned** by exactly one agent at a time.
- Job delivery is **at-least-once** (a job might execute multiple times).
- Exec blocks are treated as immutable specs; results are immutable bundles.

## State machines

### JobStatus

- `queued` -> `running` -> (`succeeded` | `failed`)
- `running` may revert to `queued` when lease expires (reconcile loop)

### StageStatus

- `pending` -> `running` -> (`succeeded` | `failed` | `needs_human` | `skipped`)

## Exec block

An exec block is a list of commands with:

- argv split into program + args
- optional relative cwd under the working dir
- optional environment overrides

Agents write:

- one stdout file per command
- one stderr file per command
- manifest.json aggregating the metadata

Additionally, agents write always-on metadata in `meta/`:

- `meta/env.json` (agent id, run/stage ids, workdir, executor)
- `meta/repo.txt` (best-effort VCS snapshot; prefers `jj`, falls back to `git`)

Manifest includes:
- start/end timestamps (unix ms)
- exit codes
- stdout/stderr file paths (relative to bundle root)

And additional fields:

- `executor`: `local` or `slurm`
- `slurm_job_id` (when applicable)
- `extra_files`: list of extra bundle files written by the runner

### Slurm executor

An exec block may set `executor.kind = "slurm"` with options (`partition`, `time_limit`, `cpus_per_task`, `mem_mb`, `extra_args`).

The agent submits a batch script via `sbatch --parsable`, polls `squeue`, and writes per-command logs into the same bundle layout.

## Safety policy (prototype)

Validation is a *pre-exec gate* applied in the daemon (before job creation)
and again in the agent (before execution).

Rules:
- Shell entrypoints (`sh`, `bash`, `cmd.exe`, `powershell`) are **blocked by default**.
  - Can be explicitly allowed via `ExecBlockSpec.allow_shell = true`.
- Obvious destructive tools are blocked or constrained to relative paths:
  - `rm`, `rmdir`, `unlink`, `dd`, `mkfs`, `shutdown`, `reboot`, etc.

The goal is not perfect security (that requires sandboxing), but to prevent
accidental foot-guns and make intent explicit.

## API (HTTP)

See `docs/API.md` for endpoints and payloads.
