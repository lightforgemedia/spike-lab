# SPL Overview

SPL is a unified control plane for **specifying**, **executing**, **validating**, and **landing** work in a repo with:

- Durable queue + leases (safe concurrency)
- Evidence-first, mechanical gates (no “trust me” merges)
- First-class HITL (ASK/DECISION/RESET as state transitions)
- Specs/requirements/use cases tied to code anchors
- Deterministic Context Packs for agents/tools (no “freestyle repo search”)
- **VCS adapters in v0: git and jj** (project chooses one)

SPL targets the “push a batch of ready work and walk away” workflow with reliable auditability:
**what ran, what changed, why it merged, and what evidence proves it**.

## Core Concepts

### Task, Revision, Run

- **Task**: identity + priority + deps + owner/tags
- **Revision**: immutable “what should be executed” (compiled from a Spec Pack)
- **Run**: one attempt of a Revision producing a complete evidence bundle

Operators should be able to reason about everything via these primitives.

### Evidence-First Gates

A change cannot land unless the required gates PASS and the required evidence artifacts exist.
Evidence is stored as:
- raw artifacts: `~/.spl/artifacts/<project>/<run_id>/...` (not committed)
- curated summaries: `.spl/reviews/...` (committed)

### HITL Is a Gate, Not a Suggestion

If SPL detects ambiguity, policy boundaries, or drift, it emits an ASK and stops.
Only a DECISION/RESET can unblock.

### VCS Support (v0)

SPL supports two VCS modes selected per project in `spl.toml`:

- `vcs.type = "git"`:
  - workspaces via git worktrees
  - snapshots via git commits
  - landing via git apply/commit (deterministic)

- `vcs.type = "jj"`:
  - requires colocated jj repo setup
  - workspaces via jj workspaces
  - snapshots via jj commits (or equivalent stable revision)
  - landing uses a unified strategy implemented by the VCS adapter (see `docs/VCS_ADAPTERS.md`)

SPL does **not** support mixing git and jj per-task in v0 (KISS). Pick one per project.

## Default Workflow

1. Create or update a Spec Pack for a task.
2. Compile Spec Pack into a pinned Revision.
3. Enqueue the Revision.
4. Workers lease queue items and execute in isolated workspaces.
5. Gates run and evidence is produced.
6. Landing lane serializes merges to main.
7. Task is marked done with a committed post-review summary.

## Evidence Layout

- `.spl/reviews/` (committed)
  - `ASK-<task>.md`
  - `<date>.<task>.post.md`
- `~/.spl/artifacts/<project>/<run_id>/` (local-only, not committed)
  - `evidence_manifest.json`
  - `spec_pack.yaml`
  - `context_pack/`
  - gate outputs (smoke/audit/review/validate/post-smoke)
  - `diff.patch`
  - `worklog.md`

## Operator UX

Pit-of-success commands:

- Run continuously:
  - `spl worker daemon`
- Watch status:
  - `spl status --watch`
- Resolve decisions:
  - `spl hitl inbox`
  - `spl hitl decide --task <id> --option <n>`

## Non-Goals (v0)

- Not a hosted service (local-first CLI)
- Not a full CI replacement
- Not a general workflow engine for arbitrary pipelines
- Not a monorepo build orchestrator (integrates with existing test commands)
- Not “auto-merge anything”: landing remains evidence-gated
