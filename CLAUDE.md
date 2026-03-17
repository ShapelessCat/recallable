# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Evaluate questions critically before responding — don't flatter or blindly comply.**

## Project Overview

Recallable is a Rust library implementing the Memento design pattern via procedural macros. It generates companion "memento" structs and state restoration logic at compile time. Two-crate workspace: `recallable` (traits) and `recallable-macro` (proc macros).

MSRV: Rust 1.85 (edition 2024). `no_std` compatible.

## Commands

```bash
cargo build                                           # Build all crates
cargo test --package recallable                       # Tests with default features (serde)
cargo test --package recallable --no-default-features # Tests without serde
cargo test --package recallable --features impl_from  # Tests with impl_from feature
cargo fmt -- --check                                  # Format check
cargo clippy --workspace --all-targets --all-features # Lint
```

CI runs the test matrix on stable and validates the MSRV on Rust 1.85.0. Coverage thresholds: 100% function, 90% line, 90% region.

## Architecture

### Trait hierarchy (`recallable/src/lib.rs`)

- `Recallable` — declares associated `type Memento`
- `Recall: Recallable` — infallible `fn recall(&mut self, memento)`
- `TryRecall: Recallable` — fallible variant with custom error; blanket impl for all `Recall` types

### Macro crate (`recallable-macro/src/`)

- `lib.rs` — three entry points: `#[recallable_model]` attribute macro, `#[derive(Recallable)]`, `#[derive(Recall)]`
- `context/context.rs` — `MacroContext` orchestrates all code generation
- `context/memento_struct.rs` — generates the companion `{Name}Memento` struct
- `context/recallable_impl.rs` — generates `Recallable` trait impl
- `context/recall_impl.rs` — generates `Recall` trait impl
- `context/from_impl.rs` — generates `From<Struct>` for memento (behind `impl_from` feature)
- `context/utils.rs` — shared helpers

### Code generation patterns

- All generated code wrapped in `const _: () = { ... }` blocks with `#[automatically_derived]`
- Automatic trait bound inference for generic type parameters
- Generic params used only by skipped fields are pruned from memento type
- `#[recallable]` fields use `<FieldType as Recallable>::Memento` in the memento struct
- `#[recallable(skip)]` fields excluded from memento; with serde feature, also get `#[serde(skip)]`
- Memento types derive `Deserialize` but not `Serialize` (by design)
- `#[recallable_model]` auto-derives `serde::Serialize` on the struct when the serde feature is
  enabled — adding a manual `#[derive(Serialize)]` is a compile error

### Cargo features

- `default = ["serde"]` — auto-derives `Deserialize` on memento types
- `impl_from` — generates `From<Struct>` for memento types
- `full = ["serde", "impl_from"]`

### Constraints

- Structs only (no enums/unions)
- No lifetime parameters
- `#[recallable]` limited to simple generic types (not `Vec<T>`)

## Testing

Tests live in `recallable/tests/`. Compile-fail UI tests use `trybuild` in `tests/ui/`. Dev dependencies include `serde_json`, `postcard` + `heapless` (binary serialization), and `anyhow`.

### Attribute macro ordering in UI tests

`#[recallable_model]` must appear **before** any attributes it needs to inspect. An attribute macro's `item` token stream only contains attributes that follow it in source order.

```rust
// Correct — recallable_model sees #[derive(Serialize)] in input.attrs
#[recallable_model]
#[derive(Serialize)]
struct Foo { ... }

// Wrong — #[derive(Serialize)] is NOT visible to recallable_model
#[derive(Serialize)]
#[recallable_model]
struct Foo { ... }
```

### trybuild feature flags

`IS_SERDE_ENABLED` is a feature on `recallable-macro`, not `recallable`. trybuild compiles test files with `recallable` at `default-features = false`, but `recallable-macro` uses its own defaults (serde enabled). Wrap serde-specific `compile_fail` entries in `#[cfg(feature = "serde")]` so they only run when the test binary is compiled with serde.

## Code Style

Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). Run `cargo fmt -- --check` and `cargo clippy --workspace --all-targets --all-features` before committing. Add doc comments with examples for public APIs.
