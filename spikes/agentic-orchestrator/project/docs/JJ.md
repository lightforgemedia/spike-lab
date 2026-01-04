# JJ workspaces (multi-agent, keep-everything mode)

This prototype assumes each **project** is a Git repository and uses **Jujutsu (jj)** in *colocated* mode for multi-agent workflows.

## What the daemon does

When an agent claims a job whose stage config is an `exec_block`, the daemon attempts to:

1. Ensure the project repo is initialized for jj:
   - `jj git init --colocate`
2. Create (or reuse) a **per-run** workspace directory:
   - `<project_root>/.orchestrator/workspaces/<run_id>/<agent_id>/`
   - `jj workspace add --name <agent_id>-<run_id> <dest>`
3. Patch `ExecBlockSpec.workdir` for the claimed job to the workspace path.

If `jj` isn't available or workspace creation fails, the daemon logs a warning and leaves `workdir` as the project root.

## Why per-run workspace

Stages within a run often depend on filesystem changes (generated code, build outputs, patched files). A per-run workspace keeps those changes in one place.

To keep the prototype simple (KISS), the daemon also **pins a run to a single agent** using an expiring `run.owner_agent` lease. This avoids subtle cross-agent state drift.

## Keep-everything

Workspaces are not deleted automatically.

This makes postmortems easy, at the cost of disk usage. A later phase should add retention policies and pruning.

## Operational notes

- `jj` workspaces assume a shared underlying repository store.
- If you intend to run compute work via Slurm, the workspace path must be visible on the compute nodes (shared filesystem).