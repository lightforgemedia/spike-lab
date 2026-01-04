# Decisions

## Why SurrealDB embedded?

- Embedded mode keeps deployment simple for a “project start”.
- SurrealDB supports graph relationships (edges) while still being document-friendly.
- Using SurrealKV keeps the data local and easy to back up.

## Why jj for multi-agent work?

- jj workspaces are designed for multiple concurrent working copies.
- Revision-aware handoff is natural: each stage starts from a specific commit id.

## Why leases + heartbeats?

- Pull-based agents are simplest to scale.
- Leases prevent duplicated execution; heartbeats extend leases during long jobs.
- This supports at-least-once semantics.

## Why store logs on disk rather than in DB?

- Keeps DB small and fast.
- Log files and artifacts are naturally file-system sized.
- DB stores pointers (paths) and summaries.
