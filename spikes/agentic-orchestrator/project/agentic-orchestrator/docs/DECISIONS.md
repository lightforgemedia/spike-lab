# Decisions

## Embedded SurrealDB

Selected SurrealDB embedded via `surrealkv://...` because it can store documents and graph edges in one engine.

## Graph-first runtime model

Workflow defs can be JSON, but *runtime dependencies* are modeled as graph edges between `stage_run` records.

## Execution as immutable bundles

Instead of streaming logs into the DB, we write execution bundles to disk and store pointers in the DB.

## Leases vs. exactly-once

At-least-once + idempotent completion is simpler and robust for v0.

## Shell blocked by default

Running `bash -c "..."` is too opaque to validate reliably.
Explicit opt-in required (`allow_shell`).
