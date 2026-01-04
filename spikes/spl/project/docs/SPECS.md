# SPL Specifications (Complete v0, Normative)

This document is the normative behavior spec for SPL v0.
Implementation MUST follow FCIS: pure decision logic in core, side effects in adapters.

---

## 1) Glossary

- **Task**: identity and planning metadata.
- **Spec Pack**: structured requirements/use cases/behavior contracts.
- **Revision**: immutable compiled execution intent derived from a Spec Pack.
- **Run**: one attempt executing a Revision and producing evidence.
- **Queue Item**: durable deliverable unit referencing a Revision.
- **Lease**: exclusive, renewable claim on a Queue Item.
- **Gate**: a stage that must PASS to continue/land.
- **Validation Rule**: deterministic check that may warn/fail independent of tests.
- **ASK/DECISION/RESET**: HITL messages that cause/resolve blocked_hitl.
- **VCS Adapter**: implementation that provides workspace/snapshot/patch/landing primitives.

---

## 2) Closed Sets

### Task Status (planning-level only)
- `draft`
- `ready`
- `blocked_hitl`
- `blocked_failure`
- `done`

Operational state is DERIVED from queue_items + leases + runs.

### Run Result
- `pass`
- `fail_gate`
- `blocked_hitl`
- `crash`

### Lanes
- `execute` (parallel)
- `land` (serialized; concurrency=1)

### Gate Names (canonical)
- `spec_compile`
- `ctx_pack`
- `pre_smoke`
- `delegate`
- `audit`
- `adversarial_review`
- `validate`
- `post_smoke`
- `land`

### VCS Type
- `git`
- `jj`

---

## 3) Project Config (spl.toml) — Required Fields

Project config determines VCS adapter and commands. See `docs/CONFIG.md` for the full schema.

Hard rules:
- VCS type is project-level in v0 (no per-task overrides).
- `spl doctor` must validate the chosen VCS mode is operable.

---

## 4) Spec Pack (YAML) — Canonical Schema

A Spec Pack MUST include:

```yaml
task: "<task-id>"
intent: "<one sentence>"

scope:
  in: [ ... ]
  out: [ ... ]

use_cases:
  - id: "UC-1"
    actor: "..."
    preconditions: [ ... ]
    steps: [ ... ]
    postconditions: [ ... ]

behavior_contracts:
  - id: "BC-1"
    anchor: "rust://crate::mod::Type::method#<sig_hash>"
    examples:
      - input: {...}
        output: {...}
    invariants:
      - "..."

acceptance:
  tests:
    - "make smoke"
  manual: []   # optional

profile: "standard"  # standard | docs | hotfix | backfill_spec

policy:
  network: "deny"         # deny | allow_readonly | allow
  allow_domains: []       # requires DECISION unless profile docs and policy is allow_readonly

gates:
  required:
    - pre_smoke
    - audit
    - adversarial_review
    - validate
    - post_smoke
```

Rules:
- `use_cases` MUST be example-driven.
- `behavior_contracts` MUST reference anchors unless profile is `backfill_spec`.
- Profiles MAY change required gates but must be explicit.

---

## 5) Revision Compilation

`spec_compile` produces:
- `spec_hash = hash(canonicalized_spec_pack_yaml)`
- `revision_id`
- `required_gates` (from profile + explicit gates)
- `required_validations` (from profile)
- `anchors` extracted from spec pack

A task can be enqueued only if:
- task status is `ready`
- a revision exists
- anchors validate OR are explicitly marked manual with WARN (per profile/policy)

---

## 6) Queue, Leases, Backoff

### Idempotency Key (required)
Recommended:
`idempotency_key = hash(task_id + revision_id + spec_hash + base_rev + lane)`

### Lease Semantics
- Only one active lease per queue item.
- Lease has TTL; it is renewable.
- If a worker crashes, lease expires; item becomes visible after backoff.

### Backoff Schedule (default)
Attempt 1: immediate  
Attempt 2: +15m  
Attempt 3: +1h  
Attempt 4+: dead-letter → blocked_failure

Retry policy is gate-dependent:
- retryable: crash, known flake signatures (configurable)
- non-retryable: policy violations, spec drift without decision

---

## 7) VCS Adapter Contract (Required in v0)

SPL MUST implement a `VcsAdapter` for both git and jj.

The contract is intentionally small (KISS):

### Operations

- `repo_root_is_clean() -> bool`
- `create_workspace(task_id) -> WorkspaceHandle`
- `workspace_is_clean(handle) -> bool`
- `get_base_rev(handle) -> RevId`
- `snapshot(handle, message) -> RevId`
- `export_patch(handle, base_rev, head_rev) -> diff.patch`
- `apply_patch_to_repo_root(diff.patch, message) -> RevId`
- `cleanup_workspace(handle)`

### Notes

- **Git adapter** uses git worktrees, snapshots via git commit.
- **JJ adapter** uses jj workspaces, snapshots via jj commit (or equivalent stable revision),
  and applies patches in a way that results in a landed change consistent with the project’s mainline.
- Exact mechanics are documented in `docs/VCS_ADAPTERS.md`.

### Delegate Restrictions (Audit-Enforced)

Delegate transcripts must not contain:
- `git commit`, `git merge`, `git rebase`, `git am`
- `jj commit`, `jj describe`, `jj rebase`, `jj squash`
- any SPL state transitions (approve/done/etc)

Worker performs snapshots and landing.

---

## 8) Run Pipeline (Canonical)

Lane `execute`:

1. `spec_compile` (if revision not pinned)
2. `ctx_pack` (Context Pack A; allow Pack B request)
3. `pre_smoke`
4. `delegate`
5. `audit`
6. `adversarial_review`
7. `validate`
8. If PASS → enqueue lane `land`

Lane `land` (serialized):

1. `apply_patch_to_repo_root`
2. `post_smoke`
3. if PASS → mark done + write post summary
4. else → blocked_failure with remediation

---

## 9) Evidence Contract (Required)

Every run MUST produce:
- `evidence_manifest.json`
- `worklog.md`
- `spec_pack.yaml` (exact input)
- `context_pack/` (or DECISION waiver)
- artifacts for each executed gate
- `diff.patch`
- recorded `vcs.type` and `workspace` metadata in manifest

Missing required artifacts = fail_gate.

---

## 10) Merge Predicate (Hard Rule)

A task may be marked `done` only if:
- a PASS land-lane run exists for the accepted revision
- all required gates PASS with required evidence roles present
- no unresolved ASK exists
- spec drift check passes (pinned spec_hash equals accepted spec_hash at land time),
  otherwise requires DECISION/RESET and a new revision

---

## 11) Meaning-Change Detection (Minimum Useful Set)

FAIL when:
- exported signature/protocol/schema changes and spec not updated
- behavior contract tests/goldens fail for referenced anchors

WARN when:
- internal signature changes
- docs drift but tests pass

DECISION overrides allowed for WARN by default; FAIL requires spec update (configurable but discouraged).

---

## 12) HITL Triggers (Mechanical)

Emit ASK and set blocked_hitl when:
- spec ambiguity (missing postconditions/examples)
- policy requires approval (network allowlist, destructive ops)
- spec drift mid-run
- landing conflict not auto-resolvable
- repeated failures threshold reached (default: 2 FAILs on same gate)
