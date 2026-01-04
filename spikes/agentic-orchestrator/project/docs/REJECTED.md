# Rejected approaches (for now)

- **Store all stdout/stderr blobs inside SurrealDB**
  - rejected: would bloat DB and complicate retention

- **Push-based scheduling**
  - rejected: agents behind NAT or with intermittent connectivity become hard

- **Allow shell commands by default**
  - rejected: makes command auditing essentially meaningless

- **Fully generic multi-parent merges**
  - rejected: needs a clear merge policy and conflict handling UX
