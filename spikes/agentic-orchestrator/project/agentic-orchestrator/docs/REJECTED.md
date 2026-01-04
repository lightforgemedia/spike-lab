# Rejected or deferred options

## External message queue (Kafka/NATS/Rabbit)

Rejected for v0 to keep operational footprint minimal. SurrealDB stores queue state.

## Full jj integration

Deferred. This prototype assumes a shared filesystem and a single project root path.
See roadmap for workspace isolation and multi-agent jj workflows.

## Streaming logs into DB

Rejected for v0: bundles are simpler, immutable, and cheaper.

## Perfect command safety

Not attempted. This requires OS sandboxing. Validation here is guardrails, not a sandbox.
