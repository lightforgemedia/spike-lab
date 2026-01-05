# Jules Agent Issues & Mitigations

Tracking issues encountered during Jules sessions to improve future prompts and workflows.

## Issue Log

| Date | Project | Issue | Root Cause | Mitigation for Future Prompts |
|------|---------|-------|------------|-------------------------------|
| 2025-01-04 | SPL | `mod tests;` added to lib.rs but tests placed in `tests/` dir | Confusion between unit tests (inline `mod tests`) vs integration tests (`tests/` directory) | Add to prompt: "Use integration tests in `tests/` directory. Do NOT add `mod tests;` to lib.rs for integration tests." |
| 2025-01-04 | SPL | Used `String` instead of `RevisionId` newtype | Didn't inspect actual type definitions | Add to prompt: "Check actual type definitions before writing tests. Use newtypes like `RevisionId::new()` not raw strings." |
| 2025-01-04 | SPL | Session completed with only 1 of 4 crates tested | Partial completion, no explanation | Add to prompt: "If you cannot complete all tasks, explain what blocked you before completing." |
| 2025-01-04 | Orchestrator | 43 consecutive failed test runs (exit 101) | Stuck in loop trying same failing approach | Add to prompt: "STOP after 3 consecutive failures. Explain the error and ask for guidance." |
| 2025-01-04 | Orchestrator | Tried adding `mod tests` inside service.rs | Didn't follow existing test patterns | Add to prompt: "Study existing test files (e.g., `tests/*.rs`) and follow that pattern exactly." |
| 2025-01-04 | Orchestrator | SurrealDB enum serialization errors | Enums need `#[serde(tag = "type")]` for SurrealDB | Add to prompt: "For SurrealDB projects, enums may need `#[serde(tag = \"type\")]` attribute." |
| 2025-01-04 | Orchestrator | Plan repeated failed approach after reset | Plan wasn't updated after failure guidance | Review plan before approving. Send correction message if plan contains known-bad approach. |
| 2025-01-05 | Orchestrator | Daemon crate untestable without SurrealDB | Complex integration dependencies | Add testability notes to jules.md. Tell Jules to skip hard-to-test crates. |
| 2025-01-05 | Orchestrator | Old session took 9.6 hours with 116 iterations | Session stuck in retry loops | Set iteration limits. Abandon stuck sessions earlier. |

## Recommended Prompt Additions

### For Rust Test Tasks

```markdown
## Test Guidelines

1. **Integration vs Unit Tests**:
   - Integration tests go in `tests/*.rs` directory
   - Unit tests go in `#[cfg(test)] mod tests { }` inside the source file
   - Do NOT add `mod tests;` to lib.rs for integration tests

2. **Follow Existing Patterns**:
   - Before writing tests, examine existing test files
   - Match the import style, test structure, and patterns used

3. **Type Safety**:
   - Check actual type definitions before using them in tests
   - Use newtypes (e.g., `TaskId::new()`) not raw primitives
   - Run `cargo check` after each file change

4. **Failure Protocol**:
   - STOP after 3 consecutive test failures
   - Report: what you tried, the exact error, what you think is wrong
   - Do NOT continue making changes without guidance

5. **SurrealDB Specifics** (if applicable):
   - Enums may need `#[serde(tag = "type")]` for serialization
   - Use `serde_json::to_value()` for complex type testing
```

### For Coverage Tasks

```markdown
## Coverage Guidelines

1. **Tool**: Use `cargo-tarpaulin` for coverage
2. **Config**: Create `tarpaulin.toml` with sensible defaults
3. **Exclusions**: Exclude test files themselves from coverage
4. **Targets**:
   - Core business logic: 80%+
   - Service layers: 70%+
   - CLI/runners: 60%+
```

## Session Monitoring Best Practices

1. **Check bash outputs** - Look for repeated failures (same command, same exit code)
2. **Set iteration limits** - Sessions shouldn't run >20 iterations without progress
3. **Send guidance early** - Don't wait for AWAITING_USER_FEEDBACK, proactively message if stuck
4. **Verify patches before applying** - Check for type mismatches, missing imports

## Pit of Success Methodology

The goal is to make the **right approach the easy one** and the **wrong approach hard**.

### Principles Applied

| Principle | Implementation |
|-----------|----------------|
| **Pre-compute context** | `lib/project-profiler.ts` auto-detects crates, newtypes, test patterns |
| **Explicit file paths** | Prompt says "create `crates/core/tests/core_tests.rs`" not "add tests" |
| **Working example first** | Extract existing test pattern and include verbatim in prompt |
| **Warn about pitfalls** | Auto-detect SurrealDB/Tokio and add warnings to prompt |
| **Failure circuit breaker** | "STOP after 3 failures" prevents infinite loops |
| **Checklist deliverables** | Clear checkboxes for what "done" means |

### CLI Tool: `add-tests`

```bash
bun run lib/cli.ts add-tests ./project --coverage core:80,daemon:70
```

This command:
1. **Profiles** the project (crates, newtypes, existing tests)
2. **Extracts** existing test patterns to include in prompt
3. **Detects** pitfalls (SurrealDB, Tokio) and adds warnings
4. **Generates** a structured prompt with explicit paths
5. **Creates** Jules session with auto-archiving

### Files

| File | Purpose |
|------|---------|
| `lib/project-profiler.ts` | Auto-detect project structure and pitfalls |
| `lib/cli.ts` (add-tests) | One-command session creation |
| `templates/add-tests-v2.md` | Detailed template with phases and checkpoints |
| `templates/project-profile.md` | Manual profiling checklist |

## Template Updates

- [x] Create `templates/add-tests-v2.md` with phased approach
- [x] Add failure protocol with 3-failure circuit breaker
- [x] Add explicit file paths and working examples
- [x] Add SurrealDB/Tokio warnings when detected
- [x] Add `add-tests` CLI command with auto-profiling
