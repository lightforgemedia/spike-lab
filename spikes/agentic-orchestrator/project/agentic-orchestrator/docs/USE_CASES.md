# Use cases

## 1) Run a codified workflow for a repo

- An intent references a project directory (git repo).
- A workflow graph defines stages like:
  - `format`
  - `build`
  - `test`
  - `lint`
- Agents execute each stage and write bundles for auditing.

## 2) Multi-agent execution

- Start multiple agents on the same host.
- Daemon queues runnable stages.
- Agents compete for jobs and execute independently.
- Leases provide at-least-once reliability.

## 3) Postmortems / reproducibility

- Each stage execution writes a bundle:
  - stdout/stderr per command
  - timestamps and exit codes in manifest
- Bundle pointers are stored in DB for discovery.

## 4) Validation and guardrails

- Structured commands make review easier.
- Validation blocks common foot-guns and surfaces warnings.

## 5) Extensible DAG workflows

- Workflow graph is stored in a graph-capable DB.
- Future: dynamic stage insertion, conditional edges, fan-out/fan-in.
