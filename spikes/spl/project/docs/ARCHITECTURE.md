# SPL Architecture (Rust, FCIS, DRY, SRP, KISS)

## 1) Architectural Style: FCIS

SPL uses Functional Core, Imperative Shell:

- **Functional Core** (pure):
  - decides next stage, required gates, whether to emit ASK, backoff, lane assignment
  - returns a list of “commands” to perform (no side effects)
- **Imperative Shell**:
  - executes commands via adapters (SQLite, filesystem, git/jj, spawning tools)

This makes core logic easy to unit test.

## 2) Workspace Layout (SRP)

Recommended Cargo workspace:

- `crates/spl-core/`
  - domain types and pure decision engine
  - validation evaluation (pure)
  - profile/gate selection (pure)

- `crates/spl-storage/`
  - repository traits + in-memory impl for tests

- `crates/spl-storage-sqlite/`
  - SQLite schema/migrations + repositories

- `crates/spl-artifacts/`
  - artifact store + manifest writer/reader
  - hashing + canonicalization

- `crates/spl-vcs/`
  - `VcsAdapter` trait + shared contract test harness

- `crates/spl-vcs-git/`
  - git implementation (worktrees, snapshot, patch export/apply)

- `crates/spl-vcs-jj/`
  - jj implementation (workspaces, snapshot, patch export/apply)
  - ensures colocated repo expectations

- `crates/spl-index/`
  - anchors + symbol map + diff

- `crates/spl-runner/`
  - stage runner mapping core commands → adapter calls
  - lane concurrency orchestration

- `crates/spl-cli/`
  - CLI parsing and UX
  - thin wiring only

## 3) Core → Shell Command Interface (KISS)

The core returns a list of commands, e.g.:

- `AcquireLease(queue_id)`
- `CreateWorkspace(task_id)`
- `BuildContextPack(task_id, revision_id)`
- `RunPreSmoke(commands[])`
- `RunDelegate(model, context_pack, spec_pack)`
- `RunAudit(...)`
- `RunReview(...)`
- `RunValidate(...)`
- `EnqueueLandLane(...)`
- `ApplyPatchAndPostSmoke(...)`
- `EmitAsk(...)`
- `RecordResult(...)`

The shell executes these with adapters and records evidence.

## 4) DRY: One Pipeline, Many Profiles

There is one canonical pipeline; differences are:
- profile gate sets
- validation strictness
- model tier preferences
- policy defaults

No second pipeline.

## 5) Concurrency Model

- N execution workers process lane `execute` in parallel.
- 1 landing worker processes lane `land` serially.
- Optional resource locks prevent unsafe parallelism (migrations, schema, etc.).

## 6) VCS Adapters (git + jj in v0)

### Design choice (KISS + testability)
- SPL supports both git and jj by implementing the same small adapter contract.
- Projects choose VCS at init time.
- The rest of SPL is VCS-agnostic.

### JJ v0 landing strategy
For v0, SPL uses **git-first deterministic landing** even in jj mode:
- work happens in jj workspaces
- snapshots and diffs come from jj
- landing applies a git-format patch to the underlying git mainline
- jj is kept consistent via colocated sync (and optional `jj git import` best-effort)

This avoids subtle jj rebase/merge behavior while keeping jj as a developer-facing workspace model.

## 7) Storage Model (SQLite)

SQLite is the source of truth for:
- tasks/revisions/runs
- queue items/leasing/backoff
- messages/decisions

Evidence lives on filesystem; SQLite stores artifact references.

## 8) Security Model (v0)

- Delegates run with restricted environment (policy-configured).
- Network denied by default.
- Any relaxation requires DECISION and is recorded.
- Evidence writers must redact sensitive patterns.

## 9) Observability (v0)

`spl status --watch` reads SQLite and shows:
- active leases + TTL
- current stage per active run
- blocked tasks with remediation one-liners
- recent completions and evidence pointers
