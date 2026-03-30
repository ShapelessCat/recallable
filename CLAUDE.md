# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Evaluate questions critically before responding — don't flatter or blindly comply.**

## Project Overview

Recallable is a Rust library implementing the Memento design pattern via procedural macros. It generates companion "memento" structs and state restoration logic at compile time. Two-crate workspace: `recallable` (traits) and `recallable-macro` (proc macros).

MSRV: Rust 1.88 (edition 2024). `no_std` compatible.

## Commands

```bash
cargo build                                           # Build all crates
cargo test --package recallable                       # Tests with default features (serde)
cargo test --package recallable --no-default-features # Tests without serde
cargo test --package recallable --features impl_from  # Tests with impl_from feature
cargo fmt -- --check                                  # Format check
cargo clippy --workspace --all-targets --all-features # Lint
make test                                             # Full feature matrix (all four combos)
make coverage                                         # Merged llvm-cov HTML + JSON reports
```

CI runs the test matrix on stable and validates the MSRV on Rust 1.88.0. Coverage thresholds: 100% function, 90% line, 90% region.

## Architecture

### Trait hierarchy (`recallable/src/lib.rs`)

- `Recallable` — declares associated `type Memento`
- `Recall: Recallable` — infallible `fn recall(&mut self, memento)`
- `TryRecall: Recallable` — fallible variant with custom error; blanket impl for all `Recall` types

### Macro crate (`recallable-macro/src/`)

- `lib.rs` — three entry points: `#[recallable_model]`, `#[derive(Recallable)]`, `#[derive(Recall)]`
- `model_macro.rs` — `#[recallable_model]` expansion: injects derives, detects duplicate `Serialize`, adds `#[serde(skip)]`
- `context.rs` — codegen facade: `analyze_item`/`analyze_recall_input`/`analyze_model_input`, feature-flag constants (`SERDE_ENABLED`, `IMPL_FROM_ENABLED`)

#### Semantic analysis (`context/internal/`)

Split by item kind with shared infrastructure:

- `shared/` — cross-cutting types and helpers
  - `item.rs` — `ItemIr` enum (`Struct(StructIr)` | `Enum(EnumIr)`), the top-level IR dispatched by `context.rs`
  - `env.rs` — `CodegenEnv` (crate paths only)
  - `fields.rs` — `FieldIr`, `FieldMember`, `FieldStrategy` (`Skip`/`StoreAsSelf`/`StoreAsMemento`), field analysis
  - `bounds.rs` — `MementoTraitSpec`, shared memento bound collection
  - `generics.rs` — generic retention fixed-point loop, `GenericDependencyCollector`
  - `codegen.rs` — `CodegenItemIr`, shared codegen helpers (`build_from_value_expr`, `build_memento_field_tokens`)
  - `lifetime.rs` — `LifetimeUsageChecker` (borrowed-field rejection)
  - `util.rs` — `crate_path` helper
- `structs/` — `StructIr` (+ `StructShape`), struct-specific bounds
- `enums/` — `EnumIr` (+ `VariantIr`, `VariantShape`), enum-specific bounds, assignment-only validation

#### Code generation (`context/`)

Each codegen module dispatches on `ItemIr` to struct/enum submodules:

- `memento/` (`structs.rs`, `enums.rs`) — companion memento type definition
- `recallable_impl/` — `Recallable` trait impl
- `recall_impl/` — `Recall` trait impl
- `from_impl/` — `From<Item>` for memento (behind `impl_from` feature)

### Code generation patterns

- All generated code uses `#[automatically_derived]` on individual impl/type items
- Automatic trait bound inference for generic type parameters
- Dependency-closed generic retention: generics are kept on the memento only if referenced by
  non-skipped fields; where-clause predicates and transitive param dependencies are propagated
  via a fixed-point loop. Unreferenced-but-retained params get a synthetic `PhantomData` marker
- `#[recallable]` fields use `<FieldType as Recallable>::Memento` in the memento struct; for bare
  type params (`T`), the shorter `T::Memento` form is used instead
- `#[recallable]` accepts any path-based field type (e.g. `Option<T>`, `Wrapper<T>`), not just
  bare type params — these get `Recallable`/`Recall` whole-type bounds
- `#[recallable(skip)]` fields excluded from memento; with serde feature, also get `#[serde(skip)]`
- `#[recallable(skip_memento_default_derives)]` suppresses default `Clone`/`Debug`/`PartialEq` derives and
  their trait bounds on the generated memento; `Deserialize` is still added when serde is enabled
- Conflicting `#[recallable]` + `#[recallable(skip)]` on the same field is a compile error
- Memento visibility matches the source struct (e.g. `pub(crate) struct` → `pub(crate) struct Memento`)
- Memento fields are always private (no visibility modifiers emitted) — mementos are opaque state tokens, not a field-inspection surface
- Memento types derive `Deserialize` but not `Serialize` (by design)
- `#[recallable_model]` auto-derives `serde::Serialize` on the struct when the serde feature is
  enabled — adding a manual `#[derive(Serialize)]` is a compile error
- Generated `recall` and `from` methods are annotated `#[inline]`
- `PhantomData` fields are omitted from the memento only when explicitly marked
  `#[recallable(skip)]`

### Enum-specific behavior

- `#[derive(Recallable)]` works on all enums — generates an enum-shaped memento with matching variants
- `#[derive(Recall)]` and `#[recallable_model]` require "assignment-only" enums: every non-marker
  variant field must be `StoreAsSelf` (no `#[recallable]` fields, no skipped fields other than
  explicitly skipped `PhantomData`)
- Complex enums (with `#[recallable]` fields) can derive `Recallable` alone and implement
  `Recall`/`TryRecall` manually
- Generated `Recall` for enums does whole-variant assignment (`*self = ...`)

### Cargo features

- `default = ["serde"]` — auto-derives `Deserialize` on memento types
- `impl_from` — generates `From<Struct>` for memento types
- `full = ["serde", "impl_from"]`

### Constraints

- Structs and enums only (no unions)
- Lifetime parameters allowed, but non-skipped fields that reference item
  lifetimes are rejected at compile time (`validate_no_borrowed_fields`)
- `#[recallable]` accepts bare type params and arbitrary path types; the macro cannot resolve types,
  so it uses heuristic path matching (e.g. `is_phantom_data` matches any path ending in `PhantomData`)
- Const generics are supported and tracked through the dependency-closure system

## Testing

Tests live in `recallable/tests/`. Compile-fail UI tests use `trybuild` in `tests/ui/`. Dev dependencies include `serde_json`, `postcard` + `heapless` (binary serialization), and `anyhow`. Examples live in `recallable/examples/`.

Fuzz harnesses live in `fuzz/fuzz_targets/` (`json_memento`, `postcard_memento`) using `libfuzzer-sys`. Run with `cargo fuzz run <target>` from the `fuzz/` directory.

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

`SERDE_ENABLED` is a feature on `recallable-macro`, not `recallable`. trybuild compiles test files with `recallable` at `default-features = false`, but `recallable-macro` uses its own defaults (serde enabled). Wrap serde-specific `compile_fail` entries in `#[cfg(feature = "serde")]` so they only run when the test binary is compiled with serde.

## Code Style

Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). Run `cargo fmt -- --check` and `cargo clippy --workspace --all-targets --all-features` before committing. Add doc comments with examples for public APIs.
