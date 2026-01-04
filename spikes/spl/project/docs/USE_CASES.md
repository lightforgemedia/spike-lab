# SPL Use Cases

This document enumerates essential SPL use cases. Many “variants” are the same run pipeline with different profiles and stop reasons.

## Essential Use Cases (irreducible set)

1. Define work (Spec Pack → compiled Revision)
2. Execute work once (single run)
3. Execute continuously (daemon)
4. Land changes (serialized)
5. Resolve blocked_failure (fix + retry)
6. Resolve blocked_hitl (ASK → DECISION/RESET)
7. Maintain repo understanding (index + context packs)
8. Detect drift (meaning-change + validations)
9. **Select VCS mode (git or jj) per project (v0)**

---

## UC-0: Initialize SPL for a repo (choose VCS)

**Actor:** Operator  
**Preconditions:** repo exists  
**Flow:**
1. `spl init`
2. Operator selects VCS mode in `spl.toml`:
   - `vcs.type = "git"` OR `vcs.type = "jj"`
3. `spl doctor` verifies tooling.
**Outcome:** Project is configured and valid.

---

## UC-1: Create a task and draft a Spec Pack

**Actor:** Owner / Orchestrator  
**Preconditions:** `spl init` done  
**Flow:**
1. `spl task add --title "..."`
2. `spl spec propose --task <id> --from intent`
3. Owner edits Spec Pack for completeness.
**Outcome:** Spec Pack exists and is ready to compile.

---

## UC-2: Compile a Spec Pack into a runnable Revision

**Actor:** Owner / Orchestrator  
**Flow:**
1. `spl spec compile --task <id>`
2. Compiler validates fields + anchors and emits:
   - `spec_hash`
   - `revision_id`
**Outcome:** Task has a new Revision; eligible for enqueue.

---

## UC-3: Run a task once (dry-run)

**Actor:** Operator  
**Flow:**
1. `spl worker run --task <id> --dry-run`
2. Shows planned stages + required gates.
**Outcome:** No state change, but a plan is printed.

---

## UC-4: Run a task once (live)

**Actor:** Operator  
**Flow:**
1. `spl worker run --task <id>`
2. Leases queue item.
3. Creates workspace (git worktree OR jj workspace depending on project VCS).
4. Builds Context Pack.
5. Runs gates and produces evidence.
6. If PASS, schedules landing lane.
**Outcome:** PASS → landed → done OR blocked.

---

## UC-5: Continuous execution (N workers + serialized landing)

**Actor:** Operator  
**Flow:**
1. `spl worker daemon --n 3`
2. Parallel workers execute leaseable items.
3. Landing lane serializes merges.
**Outcome:** Tasks progress without manual babysitting.

---

## UC-6: blocked_failure due to pre_smoke failure

**Actor:** Operator / Owner  
**Flow:**
1. Run fails at pre_smoke and emits remediation + evidence path.
2. Owner fixes root cause.
3. Retry (new run attempt).
**Outcome:** New run attempt.

---

## UC-7: blocked_hitl due to ambiguity

**Actor:** Owner  
**Flow:**
1. Worker emits ASK with options and a recommendation.
2. Owner replies: `spl hitl decide --task <id> ...`
3. SPL creates a new spec revision + task revision and re-enqueues.
**Outcome:** Work resumes safely.

---

## UC-8: Spec drift mid-run

**Actor:** System + Owner  
**Flow:**
1. Worker pins `spec_hash` at lease time.
2. Before landing, worker detects spec changed.
3. Emits ASK recommending RESET or DECISION override.
**Outcome:** prevents landing stale work.

---

## UC-9: Landing conflict

**Actor:** System + Owner  
**Flow:**
1. Execute lane PASS, land lane fails with conflict.
2. Auto-repair attempted once; if non-trivial emits ASK.
**Outcome:** prevents silent incorrect merges.

---

## UC-10: Run project in jj mode (v0)

**Actor:** Operator  
**Preconditions:**
- jj installed
- project configured with `vcs.type="jj"`
- jj is initialized in a supported colocated setup
**Flow:**
1. `spl doctor` confirms jj and colocated repo health.
2. `spl worker daemon` runs normally.
**Outcome:** identical SPL semantics, different workspace/snapshot mechanics via adapter.

Notes:
- Delegate is forbidden from running `jj commit`/`jj describe` in transcripts.
- SPL produces evidence including `vcs.type="jj"` for each run.
