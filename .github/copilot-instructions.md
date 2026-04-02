# EmmyLua Analyzer — Copilot Instructions

## Project Overview

A high-performance Lua language server (LSP), static analyzer, linter, and documentation generator written in Rust. Supports Lua 5.1–5.5 and LuaJIT with EmmyLua/Luacats annotations.

## Build & Test Commands

```bash
# Build
cargo build --release                        # Full workspace
cargo build --release -p emmylua_ls          # Single crate

# Test
cargo test                                   # All tests
cargo test -p emmylua_parser                 # Single crate
cargo test -p emmylua_code_analysis my_test  # Single test by name

# Format
cargo fmt --all

# Lint
cargo clippy

# Pre-commit hooks (trailing whitespace, symlinks, etc.)
pre-commit run --all
```

## Crate Architecture

The workspace is split into focused crates with a clear dependency flow:

```
emmylua_parser          → Lua CST via rowan (lexer → parser → green/red tree)
emmylua_parser_desc     → Markdown/RST comment highlighting extension
emmylua_code_analysis   → Core semantic engine (DbIndex, type inference, diagnostics)
emmylua_diagnostic_macro → Proc-macro: #[derive(LuaDiagnosticMacro)] for diagnostic enums
schema_to_emmylua       → JSON Schema → EmmyLua type definition converter
emmylua_ls              → LSP server (handlers per feature, async via tokio)
emmylua_check           → CLI static analyzer
emmylua_doc_cli         → CLI documentation generator
emmylua_formatter       → Code formatter
```

`tools/` contains workspace utilities: `edit_version`, `schema_json_gen`, `std_i18n`.

## Core Architecture: `emmylua_code_analysis`

### Entry Point

`EmmyLuaAnalysis` (in `lib.rs`) is the top-level struct. It wraps:
- `LuaCompilation` — owns the `DbIndex` and drives re-analysis
- `LuaDiagnostic` — runs diagnostic checkers
- `Emmyrc` — configuration

### Analysis Pipeline

1. Files enter through `Vfs` (virtual filesystem, tracks `FileId` → source text + syntax tree)
2. `LuaCompilation::update_index(file_ids)` triggers `analyzer::analyze()` which runs compilation analyzers
3. Compilation analyzers populate `DbIndex` — a collection of specialized sub-indexes:
   - `LuaDeclIndex` — declarations
   - `LuaTypeIndex` — type definitions
   - `LuaMemberIndex` — table/class members
   - `LuaReferenceIndex`, `LuaSignatureIndex`, `LuaModuleIndex`, `LuaFlowIndex`, etc.
4. `SemanticModel` provides a per-file read view over `DbIndex`; type inference runs lazily via `LuaInferCache`

### Type System

`LuaType` (in `db_index/type/types.rs`) is the core enum. Key variants:
- Primitive: `Unknown`, `Any`, `Nil`, `Boolean`, `String`, `Integer`, `Number`, `Table`, `Function`
- Const: `BooleanConst`, `StringConst(ArcIntern<SmolStr>)`, `IntegerConst(i64)`, `FloatConst(f64)`
- Composite: `Union`, `Intersection`, `Tuple`, `Array`, `Generic`, `Object`
- References: `Ref(LuaTypeDeclId)` (reference to declared type), `Def(LuaTypeDeclId)` (definition site)
- Special: `Signature`, `Instance`, `TypeGuard`, `Conditional`, `Mapped`, `Variadic`

`ArcIntern<SmolStr>` is used throughout for cheap string interning (intern once, compare by pointer).
`InFiled<T>` associates any value with a `FileId`.

### Diagnostics

Each diagnostic rule is its own file under `diagnostic/checker/`. To add a new rule:
1. Create `crates/emmylua_code_analysis/src/diagnostic/checker/<rule_name>.rs`
2. Register it in `diagnostic/checker/mod.rs`
3. Add the variant to `LuaDiagnosticCode` enum — `#[derive(LuaDiagnosticMacro)]` auto-generates `get_name()`, `FromStr`, and `Display` using kebab-case conversion of variant names (e.g., `UndefinedGlobal` → `"undefined-global"`)

### LSP Handlers

`emmylua_ls/src/handlers/` has one directory per LSP feature (completion, hover, definition, etc.). Each handler implements the corresponding LSP request/notification.

## Key Conventions

### No panics in production code
`emmylua_code_analysis/lib.rs` enforces `deny(clippy::unwrap_used, clippy::panic)` outside of `#[cfg(test)]`. Use `?`, `if let`, or explicit error handling instead of `.unwrap()` or `panic!()`.

### Testing with googletest
Use `googletest` with `#[gtest]` annotation, not `#[test]`:
```rust
use googletest::prelude::*;

#[gtest]
fn test_something() {
    assert_that!(result, eq(expected));   // preferred over assert_eq!
    expect_that!(a, ne(b));              // non-fatal; continues after failure
}
```

### `VirtualWorkspace` for analysis tests
`emmylua_code_analysis` exports `VirtualWorkspace` (in `test_lib/`) — use it for integration tests that need to analyze Lua snippets without a real filesystem.

### Rust edition & formatting
- Rust 2024 edition (`rustfmt.toml`: `edition = "2024"`)
- Line width: 100 characters
- 4 spaces, no hard tabs

### i18n
Both `emmylua_parser` and `emmylua_code_analysis` use `rust-i18n`. Locale files live under each crate's `locales/` directory. Call `set_locale()` before running analysis if locale matters.

### Clippy allowances
Several Clippy lints are `allow`-listed workspace-wide (see `Cargo.toml` `[workspace.lints.clippy]`), including `module_inception`, `enum_variant_names`, `too_many_arguments`, and `cognitive_complexity`. Do not re-enable these in individual crates.
