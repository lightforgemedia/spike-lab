# Scenarios and gap check

This file simulates common situations to validate the execution flow and identify gaps.

1. **Single stage, local, all commands succeed**
   - expected: job -> succeeded, stage -> succeeded, run -> succeeded
   - logs present per command

2. **Single stage, local, command fails**
   - expected: command status failed, job failed, stage failed, run failed
   - lease ends, no duplicate completion accepted without token

3. **Multi-command, allow_failure on first command**
   - expected: first command failed but job can still succeed if later commands succeed

4. **Two stages sequential (build -> test)**
   - expected: test stage input_revision updated from build output_revision

5. **Two agents racing to claim**
   - expected: only one claim succeeds due to conditional update
   - second agent gets no assignment or a different job

6. **Agent crashes mid-job**
   - expected: lease expires; another agent can reclaim and re-run (at-least-once)
   - gap: detecting partial side effects requires idempotent commands

7. **Agent heartbeat stops temporarily**
   - expected: if lease expires, job can be reclaimed (duplicate possible)
   - improvement: exponential backoff and clearer “lost lease” behavior

8. **Workspace path contains symlink escape**
   - expected: validator rejects if canonical path is outside workspace_root

9. **rm tries to delete / or ../**
   - expected: validator rejects absolute paths and traversal escape

10. **Slurm happy path**
    - expected: sbatch returns job id; agent polls; sacct used if available
    - per-command meta written by batch script

11. **Slurm without sacct**
    - expected: fallback parses __ORCH_OVERALL_RC marker

12. **Slurm job cancelled externally**
    - expected: sacct state not COMPLETED; exit code nonzero => job failed
    - gap: map specific Slurm states to Cancelled vs Failed

13. **Multiple dependencies (fan-in)**
    - current: blocked unless all input revisions match
    - improvement: implement jj merge policy per stage

14. **Retention GC enabled**
    - expected: old run directories deleted, keep-last-n honored
    - gap: DB records are not pruned in this prototype

15. **Remote agents without shared filesystem**
    - gap: needs artifact upload (S3 / HTTP) instead of writing to local paths

16. **Secrets injection**
    - current: not implemented; plan: per-stage secret sources and redaction

17. **Pre-exec validation false positives**
    - current: conservative; can reject safe commands
    - improvement: command-specific argument parsing or allow-list policies

18. **At-least-once idempotency**
    - current: relies on user commands being idempotent
    - improvement: stage-level idempotency keys and output caching

19. **Audit / provenance**
    - current: stores run result JSON, logs on disk
    - improvement: signed attestations + immutable artifact store

20. **Graph introspection tooling**
    - current: deps edges exist but not queried
    - improvement: queries to produce DAG visualizations and “why blocked” explanations
