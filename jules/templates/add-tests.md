Add comprehensive tests to the ${PROJECT_NAME} project.

PROJECT: ${PROJECT_PATH}

CURRENT STATE:
${CURRENT_STATE}

TASKS:
${TASKS}

## CRITICAL: Failure Protocol

**STOP after 3 consecutive test failures.** Do not continue making changes.
Instead:
1. Report the exact error message
2. Explain what you tried
3. Ask for guidance

## Test Guidelines

### Integration vs Unit Tests
- **Integration tests**: Place in `tests/*.rs` directory
  - Import crate as external: `use ${CRATE_NAME}::*;`
  - Do NOT add `mod tests;` to lib.rs
- **Unit tests**: Place inside source file
  - Use `#[cfg(test)] mod tests { use super::*; }`

### Before Writing Tests
1. **Study existing tests** - Look at any `tests/*.rs` files and follow their pattern exactly
2. **Check type definitions** - Use actual types, not primitives
   - Use `TaskId::new()` not `"task-1".to_string()`
   - Use newtypes like `RevisionId`, `QueueId`, etc.
3. **Run `cargo check`** after each file change before running tests

### SurrealDB Projects (if applicable)
- Enums may need `#[serde(tag = "type")]` for serialization
- Test enum serialization with `serde_json::to_value()`

## Coverage Setup

1. Add cargo-tarpaulin configuration:
   - Create tarpaulin.toml with settings
   - Add coverage target to justfile or Makefile
   - Document coverage commands in README

2. Coverage measurement:
   - Use `cargo tarpaulin` for coverage reports
   - Output formats: html, json
   - Exclude test code from coverage metrics

## Rules

- Run `cargo test` after each change to verify tests pass
- Do NOT modify production code unless fixing a bug found by tests
- Each test should have a clear name describing what it tests
- If you cannot complete all tasks, explain what blocked you before completing
