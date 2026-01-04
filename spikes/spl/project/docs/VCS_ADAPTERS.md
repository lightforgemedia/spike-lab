# VCS Adapters (git + jj in v0)

This document defines the intended behavior of the git and jj adapters.
Exact CLI flags may evolve; the adapter implementation must satisfy the contract in `docs/SPECS.md`.

## Shared Goals

- Provide isolated per-task workspaces.
- Allow worker-controlled snapshotting to a stable revision id.
- Export a patch from workspace changes.
- Apply patch to repo root for landing in a deterministic way.
- Provide cleanliness checks appropriate to the VCS.

## Git Adapter (v0)

### Workspace
- Uses git worktrees rooted under `.spl/workspaces/<task-id>/...`
- Base revision: the commit hash of repo root `HEAD` at workspace creation.

### Snapshot
- Worker stages all changes and creates a commit.
- Delegate is forbidden from running `git commit` (audit-enforced).

### Patch export
- Export a patch representing base..head suitable for deterministic application.

### Apply patch (landing)
- Apply patch to main branch in repo root.
- Produce one landed commit with controlled message format.

### Cleanup
- Remove worktree and prune.

## JJ Adapter (v0)

### Required repo setup
- `jj` must be installed.
- Repo must be in a supported jj setup (typically colocated with git).
- Mainline is tracked via a configured bookmark (e.g. `main`).

### Workspace
- Uses jj workspaces rooted under `.spl/workspaces/<task-id>/...`
- Base revision: stable identifier of the mainline revision at workspace creation.

### Snapshot
- Worker creates a stable revision representing the workspace changes (jj commit).
- Delegate is forbidden from running `jj commit` or related history mutation commands (audit-enforced).

### Patch export
- Export a git-format patch representing base..head.

### Apply patch (landing) — v0 policy
For v0, landing is **git-first** even in jj mode:
- apply git-format patch to the underlying git mainline
- optionally run `jj git import` best-effort to keep jj view consistent

This avoids subtle jj rebase/merge complexity while keeping jj as the workspace mechanism.

### Cleanup
- Forget jj workspace (by name) and remove directory.
- Ensure no stale workspace state remains.

## Cross-cutting Footguns (handled by doctor)

- Wrong directory: running SPL from inside a workspace.
- Dirty repo root: repo has untracked/unstaged changes that can corrupt landing.
- Missing VCS tool: git/jj not installed.
- jj not initialized or not colocated when required.

`spl doctor` must fail-fast with remediation steps.
