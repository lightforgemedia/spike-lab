# Security notes (prototype)

This project is a *developer tool prototype*, not a hardened multi-tenant service.

## Threat model (explicitly not handled)

- Hostile commands (RCE) — an agent executes commands, so you already have code execution.
- Host OS sandboxing — not implemented (use containers / VMs in production).
- Secret management — not implemented.

## What we *do* handle

### 1) Intent clarity via structured commands

We do not accept opaque shell strings as the main interface.
Commands are specified as:

- program
- args[]
- cwd (relative)
- env map

This makes auditing and diffing possible.

### 2) Pre-exec validation

Validation is applied:
- in the daemon (before enqueue)
- in the agent (before execution)

We:
- block common shell entrypoints by default
- constrain obvious destructive tools to relative paths

### 3) Immutable execution bundles

Bundles are append-only and include:
- stdout/stderr per command
- timestamps
- exit codes
- manifest.json

This supports postmortems and repeatability.

## Hardening suggestions

- Run agents in containers with read-only mounts + seccomp/apparmor
- Use a dedicated, unprivileged user account
- Add allow-lists per project for commands and network access
- Cryptographically sign bundles and push to remote object storage
