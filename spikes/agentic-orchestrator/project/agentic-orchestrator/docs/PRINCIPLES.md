# Principles followed

## KISS

- Minimal moving parts: one daemon, N polling agents, small HTTP JSON API.
- No external queues; SurrealDB stores job + lease state.

## DRY

- Shared `orchestrator-core` crate holds shared types + validation.
- Daemon and agent share the same API models.

## SRP

- Daemon: scheduling + DB ownership.
- Agent: execution + bundle writing.
- Core: logic (models, validation).

## FCIS

Failure modes are called out and surfaced:
- leases expire -> jobs re-queued
- validation blocks -> stage marked `needs_human`
- agent reports bundle paths -> stored for postmortems

## Idempotency and at-least-once

- Completion endpoint is idempotent for a given `(job_id, lease_token)`.
- Scheduler only transitions a stage to succeeded once.
