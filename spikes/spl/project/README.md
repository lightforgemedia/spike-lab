# SPL (Greenfield) — Rust Workspace

This is a **greenfield SPL** workspace implementing the architecture/specs under `docs/`.

SPL is a unified control plane for:
- Spec Packs → immutable Revisions
- Durable queue + leases
- Evidence-first gates + typed manifests
- HITL (ASK/DECISION/RESET)
- Dual VCS adapters (**git + jj**) in v0 (project-level choice)

## macOS prerequisites

- Rust toolchain (stable): https://rustup.rs
- Git (installed by default on macOS; can also use Homebrew)
- Optional (for jj mode): Jujutsu
  - `brew install jj`

## Build

From repo root:

```bash
cargo build
cargo test
```

## Try the CLI

```bash
cargo run -p spl-cli -- --help
```

## Quick start (local repo)

Inside an existing git repo:

```bash
# from repo root
cargo run -p spl-cli -- init
cargo run -p spl-cli -- doctor
```

This creates `.spl/` and a SQLite database at `.spl/spl.db`.

> Note: v0 is a foundation. Many behaviors are scaffolded but the docs and tests
> define the intended semantics. Start by running the scenario tests and VCS
> contract tests and iterate.

## Docs

See `docs/`:
- `OVERVIEW.md`
- `PRD.md`
- `SPECS.md` (normative)
- `ARCHITECTURE.md`
- `VCS_ADAPTERS.md`
- `SCENARIOS.md`

## Helpers

If you install `just`, you can use:

```bash
just test
just test-scenarios
just test-vcs
```
