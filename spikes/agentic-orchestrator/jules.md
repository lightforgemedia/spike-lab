# Jules Context: agentic-orchestrator

Spike-specific instructions for Jules AI coding sessions.

## Project Structure

```
project/
├── crates/
│   ├── core/          # Domain types, validation
│   ├── daemon/        # Orchestration service (SurrealDB)
│   └── agent/         # Worker agent
└── Cargo.toml
```

## Known Pitfalls

### SurrealDB Enum Serialization
The daemon crate uses SurrealDB. Enums like `TaskStatus`, `JobStatus` may need:
```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskStatus { ... }
```

### Tokio Async
Tests involving async code need `#[tokio::test]` not `#[test]`.

## Existing Test Pattern

Reference: `crates/core/tests/validation.rs`

```rust
use orchestrator_core::model::{CommandSpec, ExecBlockSpec};
use orchestrator_core::validation::{validate_exec_block, Decision};

#[test]
fn test_name() {
    let spec = ExecBlockSpec {
        workdir: "/tmp".into(),
        // ... use Default::default() for optional fields
    };
    let result = validate_exec_block(&spec);
    assert_eq!(result.decision, Decision::Block);
}
```

## Test File Locations

| Crate | Test Location | Notes |
|-------|---------------|-------|
| core | `crates/core/tests/*.rs` | Sync tests, use `orchestrator_core::*` |
| daemon | `crates/daemon/tests/*.rs` | May need `#[tokio::test]` |
| agent | `crates/agent/tests/*.rs` | May need `#[tokio::test]` |

## What NOT To Do

- Do NOT add `mod tests;` to `lib.rs` for integration tests
- Do NOT add inline tests to `service.rs` - use separate test files
- Do NOT use raw strings for newtypes (use `TaskId::new()` etc.)
- Do NOT try to test daemon crate directly - requires running SurrealDB
- Do NOT test agent crate without mocking - requires daemon connection

## Testability Notes

| Crate | Testable? | Notes |
|-------|-----------|-------|
| core | YES | Pure types, no DB dependency |
| daemon | HARD | Requires SurrealDB, complex setup |
| agent | HARD | Requires daemon connection |

**Recommendation**: Focus on `core` crate tests first. Daemon/agent need integration test infrastructure.

## Commands

```bash
# Check compilation
cargo check -p orchestrator-core

# Run tests for specific crate
cargo test -p orchestrator-core

# Run all tests
cargo test

# Coverage
cargo tarpaulin --out Html
```

## Coverage Targets

| Crate | Target |
|-------|--------|
| core | 80% |
| daemon | 70% |
| agent | 70% |
