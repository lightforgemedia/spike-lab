# Next steps (recommended order)

## 1) Make “intent” first-class (DB + API)
- Add `intent` table with:
  - `id`, `run_id`, `goal`, `constraints`, `created_at_ms`
- Add endpoints:
  - `GET /v1/runs/{id}/intent`
  - `PATCH /v1/runs/{id}/intent` (append notes / constraints)

## 2) Add approval flow for gate stages
- Store `gate` config explicitly (message, approvers, timeout).
- Add endpoints:
  - `POST /v1/stages/{stage_id}/approve`
  - `POST /v1/stages/{stage_id}/reject`
- Scheduler:
  - `needs_human` stages block downstream until approved.

## 3) Make stage status reflect job status
- When claiming a job, set stage to `running` (already done).
- When lease expires:
  - mark job back to `queued` (or create a new job attempt).
  - increment `attempt` and stop after `max_attempts`.

## 4) Add richer exec_attempt storage
- Persist:
  - per-command exit code + durations (already done)
  - truncated stdout/stderr summary (first N lines)
  - structured “signals” (lint errors, test failures) via regex extractors.

## 5) Cross-host artifacts
- Add an artifact store interface:
  - local filesystem backend
  - Google Drive backend (service account)
- Job complete uploads artifacts and stores a stable URL.

## 6) Proper JJ revision wiring
- Snapshot:
  - input revision(s) (parents)
  - output revision (working-copy commit id)
- Downstream:
  - use revset expressions for multiple parents
  - optional merge stage.

## 7) Hardening
- Expand pre-exec validation into a policy engine:
  - per-project allowlist
  - path allow/deny rules
  - “dangerous flags” detection (rm -rf, find -delete, etc)
- Add structured audit logs.

## 8) Observability
- Prometheus metrics:
  - queue depth, claim latency, stage duration
- Tracing spans per run/stage/job.

## 9) Multi-project + multi-tenant
- Namespaces per project
- agent capability matching (tags/labels)
