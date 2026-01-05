# Task: Add Tests to ${PROJECT_NAME}

## Project Context
- **Path**: ${PROJECT_PATH}
- **Language**: Rust
- **Test Framework**: Built-in (`#[test]`)
- **Coverage Tool**: cargo-tarpaulin

## Current State
${CURRENT_STATE}

---

## PHASE 1: Discovery (DO THIS FIRST)

Before writing ANY code, complete these steps:

### 1.1 Find Existing Test Patterns
```bash
find ${PROJECT_PATH} -name "*.rs" -path "*/tests/*" | head -5
```
If tests exist, READ them and use the SAME pattern.

### 1.2 List All Crates
```bash
ls ${PROJECT_PATH}crates/
```

### 1.3 For Each Crate, Check Exports
```bash
# Example for crate "core":
grep "^pub " ${PROJECT_PATH}crates/core/src/lib.rs
```
This tells you what types/functions are available to test.

### 1.4 Check for Newtypes
Look for patterns like:
```rust
pub struct TaskId(Uuid);
pub struct RevisionId(String);
```
These need `::new()` not raw values.

**CHECKPOINT**: List what you found before proceeding.

---

## PHASE 2: Test Structure (FOLLOW EXACTLY)

### Integration Tests (Preferred)
Create files in `${PROJECT_PATH}crates/{crate}/tests/` directory:

```
${PROJECT_PATH}
└── crates/
    └── {crate-name}/
        ├── src/
        │   └── lib.rs          # DO NOT ADD mod tests; HERE
        └── tests/
            └── {crate}_tests.rs  # CREATE THIS
```

### Integration Test Template
```rust
//! Integration tests for {crate-name}
//! File: crates/{crate-name}/tests/{crate}_tests.rs

use {crate_name}::{Type1, Type2, Type3};

#[test]
fn test_type1_creation() {
    let item = Type1 {
        id: Type1Id::new(),  // Use newtypes!
        // ... other fields
    };
    assert!(/* meaningful assertion */);
}

#[test]
fn test_type1_behavior() {
    // Test actual behavior, not just construction
}
```

### Unit Tests (Only When Needed)
Only use inline `mod tests` for testing private functions:

```rust
// Inside src/some_module.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_helper() {
        // ...
    }
}
```

---

## PHASE 3: Implementation Order

Complete in this EXACT order:

### Step 1: Create Test File
```bash
mkdir -p ${PROJECT_PATH}crates/{crate}/tests/
touch ${PROJECT_PATH}crates/{crate}/tests/{crate}_tests.rs
```

### Step 2: Add Minimal Test
```rust
use {crate_name}::*;

#[test]
fn sanity_check() {
    assert!(true);
}
```

### Step 3: Verify Compilation
```bash
cd ${PROJECT_PATH} && cargo test -p {crate-name} --no-run
```
**STOP if this fails.** Fix before adding more tests.

### Step 4: Add Real Tests (One at a Time)
After EACH test, run:
```bash
cd ${PROJECT_PATH} && cargo test -p {crate-name}
```

---

## PHASE 4: Coverage Setup

### Create tarpaulin.toml
```toml
# File: ${PROJECT_PATH}tarpaulin.toml
[tarpaulin]
out = ["Html", "Json"]
output-dir = "coverage"
exclude-files = ["tests/*", "**/tests.rs"]
ignore-panics = true
skip-clean = true
```

### Add to justfile (if exists)
```makefile
coverage:
    cargo tarpaulin --out Html --output-dir coverage/
```

---

## FAILURE PROTOCOL (MANDATORY)

### After 3 Consecutive Failures:
1. **STOP** - Do not make more changes
2. **Report**:
   - Exact error message (copy-paste)
   - What file you were modifying
   - What you were trying to do
3. **Wait** for guidance

### Common Failures & Fixes

| Error | Cause | Fix |
|-------|-------|-----|
| `expected RevisionId, found String` | Using raw string instead of newtype | Use `RevisionId::new()` |
| `file not found for module tests` | Added `mod tests;` to lib.rs for integration tests | Remove `mod tests;` - integration tests don't need it |
| `cannot find type X in this scope` | Missing import | Add to `use` statement |
| `invalid type: enum` | SurrealDB enum serialization | Add `#[serde(tag = "type")]` to enum |

---

## DELIVERABLES CHECKLIST

Before completing, verify:

- [ ] Each crate has tests in `crates/{crate}/tests/` directory
- [ ] All tests pass: `cargo test` exits 0
- [ ] No `mod tests;` added to any lib.rs (unless testing private functions)
- [ ] tarpaulin.toml created
- [ ] Coverage can run: `cargo tarpaulin --out Html`

---

## TASKS
${TASKS}

## COVERAGE TARGETS
${COVERAGE_TARGETS}
