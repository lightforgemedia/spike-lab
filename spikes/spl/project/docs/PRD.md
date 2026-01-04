# SPL Product Requirements Document (PRD)

## Summary

SPL is a unified system to define, queue, execute, validate, and land work safely in a codebase with:
- Durable queue + leases
- Deterministic evidence bundles
- Mechanical gates and validations
- First-class HITL decisions
- Specs/use cases tied to code anchors
- Deterministic Context Packs
- **VCS adapters in v0: git and jj**

## Principles (FCIS, DRY, SRP, KISS)

- **FCIS**: all workflow decisions are in a testable functional core; side effects in adapters.
- **DRY**: one canonical run pipeline; variants are profiles, not forks.
- **SRP**: each crate has one reason to change.
- **KISS**: minimal states, derived operational status, explicit invariants, small closed sets.

## Personas

### Owner
Accountable for acceptance and decisions. Responds to ASK, reviews post summaries.

### Operator
Runs daemon(s), monitors progress, handles incidents, tunes profiles.

### Delegate (automated implementor)
Implements code changes in a workspace using a Context Pack and Spec Pack.
Cannot perform workflow transitions or land merges. Must not run VCS snapshot commands.

## Goals

1. Reliability: overnight/background execution with safe concurrency.
2. Auditability: every run yields deterministic evidence; merges are explainable.
3. Anti-drift: specs/use cases remain tied to code and enforced by meaning-change + validations.
4. Simple UX: one command to run the loop, one command to watch.
5. Testability: state machine logic is unit-testable; scenario tests cover end-to-end flows.
6. Dual VCS support in v0: git and jj, both tested.

## Success Metrics

- ≥ 90% of tasks complete without human intervention when specs are unambiguous.
- 100% of landed changes have complete required evidence bundles.
- Mean time to diagnose a block ≤ 5 minutes using `spl status --watch` + evidence.
- < 5% false-positive blocks due to meaning-change rules (tunable).
- **Both VCS adapters pass the same contract test suite.**

## Functional Requirements

### FR1: Task + Revision Model
- Tasks can have multiple immutable revisions.
- Revisions are compiled from Spec Packs and pin:
  - `spec_hash`
  - profile/gates
  - required validations
  - code anchors referenced
- Runs always reference a specific revision.

### FR2: Spec System
- Structured Spec Packs include:
  - intent, scope, use cases, behavior contracts, acceptance
  - code anchor references
- `spec propose` can generate initial drafts.
- `spec compile` produces a runnable revision and fails if anchors are invalid (unless explicitly allowed).

### FR3: Index + Anchors + Context Packs
- SPL builds a repo index producing stable anchors (symbols + signatures).
- Context Packs are deterministic, budgeted, and explainable.
- Delegates run with Context Pack A (minimal) and may request Pack B (expansion).

### FR4: Durable Queue + Leases + Backoff
- Queue items are durable and have idempotency keys.
- Leases are exclusive and renewable.
- Crash recovery returns items to visibility with backoff.

### FR5: Run Pipeline + Evidence
- Runs execute a canonical stage pipeline and always emit:
  - evidence_manifest
  - worklog
  - gate artifacts
- Evidence roles are typed and required for merge.

### FR6: Mechanical Gates
Required gates (profile-dependent):
- pre_smoke
- delegate
- audit
- adversarial_review
- validate (rules engine)
- post_smoke
- land (serialized)

### FR7: HITL Lane
- ASK/DECISION/RESET are first-class objects.
- Mechanical triggers produce ASK and stop work safely.

### FR8: Landing Lane (Serialized)
- Landing is serialized; conflicts/tests failures produce explicit outcomes.
- Landing attempts are evidence-bearing runs (or sub-runs) with remediation hints.

### FR9: VCS Adapters (git + jj in v0)
- SPL supports two project-level VCS modes:
  - `git` adapter
  - `jj` adapter
- Both adapters implement the same `VcsAdapter` contract.
- Both must support:
  - workspace creation/cleanup
  - snapshotting changes to a stable revision id
  - exporting a patch/diff
  - applying the patch to the repo root for landing (adapter-defined)
  - cleanliness checks for repo root
- Delegate is forbidden from invoking snapshot/commit operations in either VCS.

## Non-Functional Requirements

- Local-first: SQLite + filesystem artifacts.
- Deterministic outputs: identical inputs should produce identical manifests (except timestamps).
- Secure-by-default: no secret exfiltration; policy controls.
- Performance: status watch updates within 1–2s; runs can be long.
- CI: must run tests for core + adapters; jj job may be separate but required for merges.

## Constraints

- Operates from repo root.
- Uses existing project test commands via config.
- jj requires a supported local setup (colocated repo), documented in `docs/VCS_ADAPTERS.md`.

## Phased Delivery (tests baked into phases)

Each phase MUST add/extend tests before adding features.

### Phase A — Core Data + FCIS State Machine
Deliver:
- core types: Task, Revision, Run, QueueItem, Lease, ASK/Decision
- SQLite schema + migrations
- pure “decide next stage” engine (functional core)
Tests:
- unit tests for state transitions
- property tests for invariants (no impossible transitions)

### Phase B — Evidence Contract + Manifest
Deliver:
- artifact store layout + manifest writer/reader
- typed evidence roles
Tests:
- manifest schema tests
- golden tests for manifest determinism

### Phase C — Queue + Leases + Backoff
Deliver:
- enqueue/claim/renew/release, crash recovery
Tests:
- concurrency tests (lease exclusivity)
- backoff schedule tests

### Phase D — VCS Layer (git + jj)
Deliver:
- `VcsAdapter` trait + **two implementations**:
  - git worktree adapter
  - jj workspace adapter
- adapter contract test harness (shared)
Tests:
- adapter contract tests for git (required)
- adapter contract tests for jj (required; may run in a dedicated CI job that installs jj)

### Phase E — Run Pipeline + Gates (smoke/audit/review)
Deliver:
- stage runner + evidence outputs
- profiles: standard/docs
Tests:
- scenario tests: happy path + blocked_failure + retry

### Phase F — Landing Lane + Conflict Handling
Deliver:
- serialized landing with explicit outcomes
Tests:
- scenario tests: landing conflict, landing test fail

### Phase G — Index + Anchors + Context Packs
Deliver:
- index build, anchors, ctx pack A/B
Tests:
- index correctness tests on fixtures
- ctx budget tests + explain output tests

### Phase H — Meaning-Change + Validation Engine
Deliver:
- validations: meaning-change/spec coverage/policy/structural
Tests:
- scenario tests: exported signature change requires spec update/decision

## Acceptance Criteria (v0)

- Operators can run: `spl worker daemon` and complete at least 10 tasks end-to-end on fixtures.
- Every landed change has a manifest and required evidence roles.
- ASK/DECISION unblocks a blocked_hitl state deterministically.
- Crash recovery does not double-land or lose queue items.
- **Both git and jj adapters pass the same adapter contract suite.**
