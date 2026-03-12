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
cargo clippy --verbose                                # Lint
```

CI runs all three test configurations. Coverage thresholds: 100% function, 90% line, 90% region.

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

## Code Style

Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). Run `cargo fmt` and `cargo clippy` before committing. Add doc comments with examples for public APIs.
