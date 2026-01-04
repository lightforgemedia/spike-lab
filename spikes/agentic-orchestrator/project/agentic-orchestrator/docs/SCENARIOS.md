# Scenarios (10–20) and gap analysis

This doc simulates typical and edge cases and checks expected behavior.

## 1) Single stage succeeds

- Workflow: one exec_block stage
- Expected: stage_run succeeded, run succeeded, bundle written

## 2) Command fails with halt_on_error=true

- Expected: command #k non-zero -> stage failed; downstream skipped

## 3) Command fails with halt_on_error=false

- Expected: continue running; overall status failed (v0) but bundle includes all commands

## 4) Two stages A->B, A succeeds

- Expected: B becomes runnable after A completion

## 5) Two stages A->B, A fails

- Expected: B skipped; run failed

## 6) Agent crashes mid-command

- Expected: lease expires; job re-queued; new agent can re-run (at-least-once)

## 7) Agent reports completion twice (retry)

- Expected: completion is idempotent and does not duplicate transitions

## 8) Two agents contend for same job

- Expected: mutex + lease update ensures only one gets it

## 9) Shell command without allow_shell

- Expected: validation blocks; stage marked needs_human

## 10) rm with absolute path

- Expected: blocked

## 11) rm with relative path inside workdir

- Expected: allowed (with warning) if paths are relative and no '..'

## 12) mv with '../' in args

- Expected: blocked (possible escape)

## 13) Long-running command

- Expected: lease heartbeats are not implemented yet (gap); recommend adding heartbeat

## 14) Daemon restarts while jobs running

- Expected: on restart, daemon can reconcile expired leases (partial in v0)

## 15) DB file moved/corrupt

- Expected: daemon errors at startup (gap: backups/migrations)

## 16) Parallel stages (A and B both depend on none)

- Expected: both jobs can be queued; N agents can work in parallel (v0 supports)

## 17) Stage produces huge stdout

- Expected: written to file (ok), but disk usage may grow (gap: retention policy)

## 18) Non-UTF8 output

- Expected: logs stored as raw bytes (we write bytes); manifest remains valid

## 19) Intent points to different project_path

- Expected: exec runs in that directory; validation uses that as boundary

## 20) Unknown executable

- Expected: spawn error -> stage failed; bundle contains error in manifest

## Gaps (v0) and suggested improvements

- Add agent heartbeats to extend leases for long-running commands
- Add explicit retention/GC for bundles
- Add workspace isolation to prevent multi-agent stomping
- Add stronger policy controls and a deny-by-default mode
- Add schema migrations and versioning
