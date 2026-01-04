# Anchors (v0)

This doc defines the v0 anchor format and signature hashing.

## Goals
- Stable identifiers for code surfaces referenced by Spec Packs.
- Deterministic hashing to support meaning-change detection.
- KISS: start narrow; expand later.

## Anchor URI format (v0)

### Rust
`rust://<crate>::<module_path>::<Item>[::<method>]#<sig_hash>`

Example:
`rust://spl_core::engine::Engine#d34db33f...`

### TypeScript (placeholder v0)
`ts://<file_path>::<symbol>#<sig_hash>`

## Signature hash (`sig_hash`)

- Hash algorithm: SHA-256
- Input: a canonical signature string
- Encoding: lowercase hex

### Canonical signature string (Rust v0)
- For functions: `fn <name>(<arg_types...>) -> <ret_type>`
- For methods: `fn <Type>::<name>(<arg_types...>) -> <ret_type>`
- Whitespace normalized to single spaces
- Type paths normalized to `::` separators

> v0 note: Rust canonical signatures in SPL are best-effort and may be refined.
> Meaning-change in v0 should focus on exported surface changes and contract tests.

## Exported surface (v0)
- Rust: `pub` items in `lib.rs` and re-exported public API modules.
- TS: exported symbols (ESM `export`) once implemented.

## Implementation notes
- `spl-index` produces anchors and `sig_hash` values.
- `spl-spec` validates anchors by querying the index; missing anchors fail compile unless profile allows manual anchors with WARN.
