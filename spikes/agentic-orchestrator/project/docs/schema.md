# Data model (SurrealDB)

Tables:
- `project`
- `run`
- `stage`
- `job`
- `agent`

Edge table (graph relationship):
- `depends_on` (created via `RELATE stage:<id>->depends_on->stage:<id>`)

See `crates/orchestrator-daemon/schema.surql` for the indexes.

## Stage dependency graph

We store dependencies in two ways:
1. `stage.deps` array: quick evaluation in the scheduler
2. `depends_on` edges: future-proof for graph traversal queries

This is slightly redundant, but keeps scheduling logic KISS while still
enabling graph-style introspection and tooling later.
