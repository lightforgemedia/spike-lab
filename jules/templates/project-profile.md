# Project Profile Template

Use this to gather context BEFORE creating a Jules session.

## Basic Info
- **Project Path**:
- **Language**: Rust | TypeScript | Go
- **Build Tool**: cargo | bun | go

## Crate/Package Structure
```
# Run: ls {path}/crates/ OR ls {path}/packages/
```

## Existing Test Patterns
```
# Run: find {path} -name "*test*.rs" -o -name "*.test.ts" | head -10
```

## Public API Surface
```
# For Rust: grep "^pub " {path}/crates/*/src/lib.rs
# For TS: grep "export " {path}/src/index.ts
```

## Newtypes/Special Types
```
# Look for: pub struct FooId(...)
```

## Dependencies to Note
- SurrealDB? → Enum serialization issues
- Tokio? → Async test setup needed
- External APIs? → Mock setup needed

## Existing Tests to Reference
```
# Paste example of working test from this project
```
