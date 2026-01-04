# SPL Configuration (spl.toml)

This document defines the project config file. SPL loads `spl.toml` from repo root.

## Goals

- KISS: minimal required keys.
- Explicitness: VCS choice is project-level in v0.
- Testability: config is validated by `spl doctor`.

## Minimal Example

```toml
[project]
id = "my-repo"
main_ref = "main"          # git branch or jj bookmark mainline
artifact_root = "~/.spl/artifacts"

[vcs]
type = "git"               # "git" | "jj"

[workspace]
root = ".spl/workspaces"   # where per-task workspaces are created
cleanup_on_success = true

[commands]
pre_smoke = ["true"]
post_smoke = ["true"]

[index]
enabled = true
languages = ["rust", "ts"]

[policy]
network_default = "deny"   # deny | allow_readonly | allow
```

## VCS Section

### git

```toml
[vcs]
type = "git"
git_main_branch = "main"
```

### jj (v0)

```toml
[vcs]
type = "jj"
jj_main_bookmark = "main"
jj_require_colocated = true
```

`spl doctor` must verify:
- `jj` is installed and runnable
- repo is initialized in a supported colocated configuration if `jj_require_colocated = true`

## Profiles (optional)

Profiles define gate sets and validation strictness.

```toml
[profiles.standard]
required_gates = ["pre_smoke","audit","adversarial_review","validate","post_smoke"]

[profiles.docs]
required_gates = ["audit","adversarial_review"]
```

If omitted, SPL provides built-in defaults.

## Flake Policy (optional)

```toml
[flakes]
auto_retry_count = 1
known_signatures = ["timeout in test_x", "connection reset by peer"]
```

## Resource Locks (optional)

```toml
[resources]
keys = ["db/migrations", "api/schema"]
```
