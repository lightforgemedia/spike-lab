# Safety model

This project provides **guardrails**, not a sandbox.

## What is enforced today

- The agent validates each command before execution.
- A small hard deny-list blocks commands that are almost always destructive in CI
  (e.g. `sudo`, `mkfs`, `dd`, `mount`, `umount`).
- For file-destructive utilities (`rm`, `rmdir`, `mv`, `cp`), we do best-effort checks:
  - absolute paths are denied
  - `..` traversal that would escape the workspace is denied
  - if a target path exists, we canonicalize it to prevent symlink escapes

## Shell commands

Shell invocations (`bash`, `sh`, etc.) can hide arbitrary behavior.
By default they are **blocked**.

You can opt-in per command with:
```json
{ "program": "bash", "args": ["-lc","..."], "allow_shell": true }
```

## What is *not* enforced

- No seccomp / container isolation
- No network policy
- No CPU/memory isolation for Local executor
- No inspection of scripts executed inside a shell

For higher assurance, run the agent inside containers or under a dedicated unprivileged user.
