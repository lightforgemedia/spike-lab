# Jules Context: SPL (Spec Pipeline Language)

Spike-specific instructions for Jules AI coding sessions.

## Project Structure

```
project/
├── crates/
│   ├── spl-core/           # Domain types (Task, Queue, Run, etc.)
│   ├── spl-spec/           # Spec parsing and compilation
│   ├── spl-storage-sqlite/ # SQLite storage implementation
│   └── spl-runner/         # Pipeline execution
├── fixtures/
│   └── scenarios/          # Test fixtures (spec_pack.yaml files)
└── Cargo.toml
```

## Known Pitfalls

### Newtypes
This project uses newtypes extensively. Always use `::new()`:
```rust
// CORRECT
let task_id = TaskId::new();
let revision_id = RevisionId::new();
let queue_id = QueueId::new();

// WRONG - will not compile
let task_id = "task-1".to_string();
```

### Integration Tests Pattern
Do NOT add `mod tests;` to `lib.rs`. Integration tests import externally:
```rust
// crates/spl-core/tests/core_tests.rs
use spl_core::{Task, TaskStatus, TaskId, RevisionId};
```

## Existing Test Pattern

Reference: `crates/spl-core/tests/core_tests.rs`

```rust
use spl_core::{Task, TaskStatus, TaskId, QueueItem, QueueId, Lane, RevisionId};

#[test]
fn test_task_creation() {
    let task = Task {
        id: TaskId::new(),
        title: "Test Task".to_string(),
        status: TaskStatus::Draft,
        priority: 1,
        tags: vec!["test".to_string()],
    };
    assert_eq!(task.title, "Test Task");
    assert_eq!(task.status, TaskStatus::Draft);
}
```

## Test File Locations

| Crate | Test Location | Notes |
|-------|---------------|-------|
| spl-core | `crates/spl-core/tests/*.rs` | Use newtypes |
| spl-spec | `crates/spl-spec/tests/*.rs` | Use fixtures |
| spl-storage-sqlite | `crates/spl-storage-sqlite/tests/*.rs` | Async, temp DB |
| spl-runner | `crates/spl-runner/tests/*.rs` | Use fixtures |

## Test Fixtures

Use fixtures in `fixtures/scenarios/` for integration tests:
```rust
let spec_path = "fixtures/scenarios/SC-01-happy-path/spec_pack.yaml";
```

## What NOT To Do

- Do NOT add `mod tests;` to `lib.rs` for integration tests
- Do NOT use `String` for `RevisionId`, `TaskId`, etc.
- Do NOT modify production code unless fixing a bug

## Commands

```bash
# Check compilation
cargo check -p spl-core

# Run tests for specific crate
cargo test -p spl-core

# Run all tests
cargo test

# Coverage
cargo tarpaulin --out Html
```

## Coverage Targets

| Crate | Target |
|-------|--------|
| spl-core | 80% |
| spl-spec | 80% |
| spl-storage-sqlite | 70% |
| spl-runner | 60% |
