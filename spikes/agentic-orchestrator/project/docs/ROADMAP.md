# Roadmap

Near-term:
- Add a first-class workflow definition (separate from jobs) and version it.
- Implement “fan-in” stages with a configurable merge policy (jj merge).
- Better Slurm state mapping: Cancelled vs Failed vs Preempted.
- Artifact upload path for remote agents (S3 / HTTP chunked).
- AuthN/AuthZ for daemon endpoints.

Medium-term:
- Web UI / graph visualization.
- Secret injection with redaction.
- Policy-as-code for command safety.
- Pluggable executors (K8s Jobs, Nomad, SSH).
