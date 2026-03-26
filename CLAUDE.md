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

- `lib.rs` — three entry points: `#[recallable_model]` attribute macro, `#[derive(Recallable)]`, `#[derive(Recall)]`
- `model_macro.rs` — `#[recallable_model]` expansion: injects derives, detects duplicate `Serialize`, adds `#[serde(skip)]`
- `context.rs` — codegen facade: re-exports IR types from `internal`, owns feature-flag constants, hosts codegen submodules
- `context/internal.rs` — re-export hub for the `internal` submodules
- `context/internal/ir.rs` — `StructIr` (semantic IR, built by `StructIr::analyze()`), `CodegenEnv`, `FieldIr`, `FieldMember`, `FieldStrategy`, `StructShape`
- `context/internal/bounds.rs` — `MementoTraitSpec`, `collect_recall_like_bounds`, `collect_shared_memento_bounds`
- `context/internal/generics.rs` — `GenericParamPlan`, `GenericDependencyCollector`, `is_generic_type_param`, generic retention fixed-point loop
- `context/internal/fields.rs` — field analysis: `has_recallable_skip_attr`, field strategy classification
- `context/internal/lifetime.rs` — `LifetimeUsageChecker` (`syn::Visit` walker for borrowed-field rejection)
- `context/internal/util.rs` — `crate_path` helper
- `context/memento_struct.rs` — generates the companion `{Name}Memento` struct
- `context/recallable_impl.rs` — generates `Recallable` trait impl
- `context/recall_impl.rs` — generates `Recall` trait impl
- `context/from_impl.rs` — generates `From<Struct>` for memento (behind `impl_from` feature)

#### IR types (in `context/internal/`)

- `StructIr` — the sole IR; built by `StructIr::analyze()` from `DeriveInput`. Holds fields, generics plan, memento name, visibility, shape
- `CodegenEnv` — resolved once per invocation: crate paths only (`recallable_trait`, `recall_trait`)
- `MementoTraitSpec` — centralizes memento derive attributes and trait bounds (common traits + optional serde)
- `FieldIr` — per-field: strategy (`Skip`/`StoreAsSelf`/`StoreAsMemento`), member accessor, memento index
- `StructShape` — `Named`/`Unnamed`/`Unit`
- `FieldStrategy` — `Skip`/`StoreAsSelf`/`StoreAsMemento`
- `GenericParamPlan` — per generic param: `Dropped`/`Retained`/`RetainedAsRecallable`
- `GenericDependencyCollector` — `syn::Visit` walker that collects which generic params a type/predicate depends on
- `LifetimeUsageChecker` — `syn::Visit` walker that detects struct lifetime usage in field types

Code generation is free functions (`gen_memento_struct`, `gen_recallable_impl`, `gen_recall_impl`, `gen_from_impl`) that take `&StructIr` + `&CodegenEnv`. Feature flags (`SERDE_ENABLED`, `IMPL_FROM_ENABLED`) are module-level constants in `context.rs`, not part of `CodegenEnv`.

### Code generation patterns

- All generated code wrapped in `const _: () = { ... }` blocks with `#[automatically_derived]`
- Automatic trait bound inference for generic type parameters
- Dependency-closed generic retention: generics are kept on the memento only if referenced by
  non-skipped fields; where-clause predicates and transitive param dependencies are propagated
  via a fixed-point loop. Unreferenced-but-retained params get a synthetic `PhantomData` marker
- `#[recallable]` fields use `<FieldType as Recallable>::Memento` in the memento struct; for bare
  type params (`T`), the shorter `T::Memento` form is used instead
- `#[recallable]` accepts any path-based field type (e.g. `Option<T>`, `Wrapper<T>`), not just
  bare type params — these get `Recallable`/`Recall` whole-type bounds
- `#[recallable(skip)]` fields excluded from memento; with serde feature, also get `#[serde(skip)]`
- `#[recallable(memento_derive_off)]` suppresses default `Clone`/`Debug`/`PartialEq` derives and
  their trait bounds on the generated memento; `Deserialize` is still added when serde is enabled
- Conflicting `#[recallable]` + `#[recallable(skip)]` on the same field is a compile error
- Memento visibility matches the source struct (e.g. `pub(crate) struct` → `pub(crate) struct Memento`)
- Memento fields are always private (no visibility modifiers emitted) — mementos are opaque state tokens, not a field-inspection surface
- Memento types derive `Deserialize` but not `Serialize` (by design)
- `#[recallable_model]` auto-derives `serde::Serialize` on the struct when the serde feature is
  enabled — adding a manual `#[derive(Serialize)]` is a compile error
- Generated `recall` and `from` methods are annotated `#[inline]`
- `PhantomData` fields in structs with lifetimes are auto-skipped from the memento

### Cargo features

- `default = ["serde"]` — auto-derives `Deserialize` on memento types
- `impl_from` — generates `From<Struct>` for memento types
- `full = ["serde", "impl_from"]`

### Constraints

- Structs only (no enums/unions)
- Lifetime parameters allowed on the struct, but non-`PhantomData` fields that reference struct
  lifetimes are rejected at compile time (field-level `validate_no_borrowed_fields`)
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
