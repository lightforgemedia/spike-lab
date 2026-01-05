# Task: Add Tests to ${PROJECT_NAME}

## Project Context
- **Path**: ${PROJECT_PATH}
- **Language**: ${LANGUAGE}

${SPIKE_SPECIFIC_CONTEXT}

---

## General Test Guidelines

### Phase 1: Discovery (DO THIS FIRST)

Before writing ANY code:
1. Find existing test files and study their patterns
2. List all modules/crates/packages
3. Check for newtypes and special types
4. Note any framework-specific requirements

**CHECKPOINT**: Report what you found before proceeding.

### Phase 2: Test Structure

**Integration Tests** (preferred for public API testing):
- Place in `tests/` directory at crate/package level
- Import module as external dependency
- Do NOT modify lib.rs/index.ts to add test modules

**Unit Tests** (for private function testing):
- Place inside source file with `#[cfg(test)]` or equivalent
- Use `mod tests { use super::*; }` pattern

### Phase 3: Implementation

1. Create test file with minimal sanity test
2. Verify compilation: `${BUILD_CHECK_CMD}`
3. Add real tests ONE AT A TIME
4. Run tests after EACH addition: `${TEST_CMD}`

---

## CRITICAL: Failure Protocol

**STOP after 3 consecutive failures.** Do not continue making changes.

Instead:
1. Report the exact error message (copy-paste)
2. Explain what you were trying to do
3. Wait for guidance

---

## Deliverables

- [ ] Tests exist for each module/crate
- [ ] All tests pass
- [ ] Coverage config created (if applicable)

Do NOT modify production code unless fixing a bug found by tests.
