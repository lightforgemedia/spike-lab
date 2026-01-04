# SPL Test Plan (Tests Baked Into Phases)

This plan ensures SPL is testable by construction and remains reliable as features are added.

## Guiding Idea (FCIS)

- Unit tests cover the functional core:
  - “Given state S and input I, the next command list is X.”
- Integration tests cover adapters:
  - SQLite, artifacts, git/jj VCS adapters
- Scenario tests cover end-to-end use cases.

## 1) Test Pyramid

### A) Unit tests (fast, deterministic)
Target: `spl-core`

- State transition tests:
  - leasing rules, backoff computation
  - HITL trigger detection
  - merge predicate evaluation
  - profile → required gates selection
- Validation engine tests:
  - meaning-change fail/warn logic
  - spec coverage enforcement
- Property tests:
  - “no task can be done without a PASS land run”
  - “no double-lease”
  - “spec drift requires ASK”

### B) Adapter contract tests (shared harness) — REQUIRED
Target: `spl-vcs` + implementations

Create one contract test suite `vcs_contract_tests` that runs against both adapters.

Contract cases (minimum):
1. Create workspace → workspace exists and isolated
2. Snapshot produces stable rev id
3. Export patch is non-empty after changes
4. Apply patch advances repo root mainline
5. Cleanup removes workspace
6. Cleanliness checks behave correctly

The same test logic must run for:
- `spl-vcs-git`
- `spl-vcs-jj`

CI requirement:
- either install jj and run in one pipeline,
- or run jj tests in a separate required job.

### C) Adapter integration tests (temp dirs)
Targets:
- `spl-storage-sqlite`
- `spl-artifacts`
- `spl-vcs-git`
- `spl-vcs-jj`

Examples:
- SQLite migrations apply from empty DB
- manifest writer emits required roles and hashes
- git adapter operates on a temp git fixture repo
- jj adapter operates on a temp jj-colocated fixture repo (or creates one in temp dir)

### D) Scenario tests (system-level)
Target: `spl-runner` + fixtures

Scenario suite MUST include:

1. Happy path: compile → run → land → done
2. pre_smoke fail → blocked_failure with remediation
3. spec ambiguity → blocked_hitl ASK emitted
4. DECISION creates new revision and resumes
5. landing conflict → ASK emitted
6. crash recovery: expired lease re-queues
7. meaning-change on exported symbol blocks unless spec updated

Each scenario asserts:
- DB rows (runs, artifacts, messages)
- required evidence roles present
- correct final task status
- deterministic remediation output

Scenario tests should run under both VCS modes:
- a subset under git (required for quick CI)
- a subset under jj (required overall; may be separate job)

## 2) Tests Embedded in the Phases (DoD)

### Phase A DoD
- core state machine has robust unit/property coverage

### Phase B DoD
- manifest schema test + golden manifest tests

### Phase C DoD
- concurrency lease test using threads

### Phase D DoD (VCS adapters: git + jj)
- contract tests pass for git adapter
- contract tests pass for jj adapter
- `spl doctor` tests cover VCS detection failures

### Phase E DoD
- scenario tests (happy path, blocked_failure, retry) under git

### Phase F DoD
- landing conflict scenario under git

### Phase G DoD
- context pack budget + explain tests

### Phase H DoD
- meaning-change blocking scenario under git and jj (required)

## 3) CI Requirements

Minimum CI steps:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

Recommended CI matrix:
- Job A: core + git adapter + scenario suite (fast)
- Job B: jj adapter contract + key scenarios (required)

## 4) Fixtures

Provide:
- `fixtures/repo_git_simple/` (tiny repo for git tests)
- `fixtures/repo_jj_simple/` (tiny repo for jj tests; or created on the fly)
- `fixtures/repo_conflict/` (landing conflict)
- `fixtures/repo_meaning_change/` (exported signature change)
- `fixtures/spec_packs/` (valid/invalid packs)

Fixtures must run offline.
