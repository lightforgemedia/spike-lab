# SPL Scenario Suite (System Tests)

This document defines SPL v0 **scenario fixtures**: deterministic, offline system tests that validate SPL semantics end-to-end across:

- Task → Spec Pack → Revision → Queue → Lease → Run pipeline → Evidence → Landing → Done/Blocked
- Mechanical gates (pre_smoke, audit, adversarial_review, validate, post_smoke)
- HITL (ASK/DECISION/RESET)
- Crash recovery + backoff
- Resource locks
- Context Packs (budget + explain)
- Meaning-change + spec drift
- **VCS adapters: git + jj**

These scenarios are the **system-level regression net** that prevents the implementation from drifting away from the normative `docs/SPECS.md`.

---

## 0) How scenario tests work (deterministic + offline)

### 0.1 Harness contract (high level)

Each scenario fixture provides:
- a seed repo (or reference to a shared fixture repo)
- a Spec Pack
- scripted side-effect outputs for gates (pre_smoke/audit/review/validate/post_smoke)
- an optional delegate patch to apply in the workspace
- expected final state and key assertions

The harness runs SPL like a real operator would:
1. Initialize project config with `vcs.type` = `git` or `jj`.
2. Load the scenario Spec Pack.
3. Compile to an immutable revision (unless scenario is about compile failure).
4. Enqueue and run the execute lane (worker).
5. If execute PASS, enqueue land lane and run landing worker (serialized).
6. Collect evidence artifacts and assert required roles exist.
7. Assert DB state, messages (ASK), and final task status.

### 0.2 Deterministic gate runners (no external tools)

For scenario tests, SPL MUST support a “test toolchain mode” where:
- **Delegate** does not call an LLM. It applies `delegate.patch` (or performs scripted file edits).
- **Audit/Review** do not call external tools. They read fixture verdict files.
- **Smoke** is simulated by fixture outputs (or run `cargo test` on small fixtures if desired).
- **Validate** reads fixture `validate.json` OR evaluates rules against fixture index diffs.

### 0.3 Suggested fixture directory layout

```text
fixtures/scenarios/
  SC-01-happy-path/
    spec_pack.yaml
    delegate.patch
    gates/
      pre_smoke.txt
      audit.json
      review.json
      validate.json
      post_smoke.txt
    expected.yaml

  SC-09-landing-conflict/
  ...

fixtures/repos/
  repo_simple/
  repo_conflict/
  repo_meaning_change/
  repo_index_gaps/
```

---

## 1) Scenario matrix (what runs under git vs jj)

Legend:
- **BOTH**: must run under git and jj (adapter parity)
- **GIT**: only required under git
- **JJ**: only required under jj

| ID | Name | Purpose | VCS |
|---:|------|---------|:---:|
| SC-01 | Happy path end-to-end | baseline pipeline correctness | BOTH |
| SC-02 | Docs profile | lighter gates, still evidence-correct | BOTH |
| SC-03 | pre_smoke failure blocks | blocked_failure + remediation | BOTH |
| SC-04 | Flake auto-retry then pass | retry policy before blocking | BOTH |
| SC-05 | Audit violation (delegate tries VCS snapshot) | audit gate correctness | BOTH |
| SC-06 | Review FAIL triggers spec hardening loop | spec ambiguity surfaced | BOTH |
| SC-07 | Meaning-change exported signature blocks | anti-drift enforcement | BOTH |
| SC-08 | Spec drift mid-run triggers ASK | prevents landing stale spec | BOTH |
| SC-09 | Landing conflict triggers ASK | landing lane semantics | BOTH |
| SC-10 | Post-smoke failure blocks | catch regressions after landing | BOTH |
| SC-11 | Crash recovery re-queues | lease expiry + backoff | BOTH |
| SC-12 | Resource lock contention | safe concurrency | BOTH |
| SC-13 | Context pack budget fallback + explain | ctx system correctness | BOTH |
| SC-14 | Anchor missing at compile time | index/anchor failure | BOTH |
| SC-15 | jj doctor failure cases | jj setup validation | JJ |
| SC-16 | git doctor failure cases | git cleanliness/worktree traps | GIT |

---

## 2) Common assertions

- Required evidence roles are present for each run (execute and land lanes).
- No DONE without PASS land-lane run.
- No unresolved ASK when marking DONE.
- spec_hash is pinned and recorded in manifest.

---

## 3) Scenario definitions

See the detailed scenario descriptions in this doc.
Implement these as fixtures plus a deterministic test harness in `spl-runner`.
