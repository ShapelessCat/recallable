# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `#[recallable(memento_derive_off)]` struct-level attribute to suppress default
  `Clone`/`Debug`/`PartialEq` derives on the generated memento
- `[package.metadata.docs.rs]` for docs.rs all-features builds
- "Generated Code" section in README showing `cargo expand` output
- `cargo doc` check in CI

### Changed

- `MementoTraitSpec` centralized: memento derive attributes and trait bounds are now
  managed in one place instead of scattered across codegen modules
- `CodegenEnv` simplified: feature flags (`SERDE_ENABLED`, `IMPL_FROM_ENABLED`) are now
  module-level constants in `context.rs`, not part of `CodegenEnv`
- Memento type visibility now matches the source struct (e.g. `pub(crate) struct` →
  `pub(crate) struct Memento`)
- Macro internals extracted from monolithic `context.rs` into focused submodules under
  `context/internal/` (ir, bounds, generics, fields, lifetime, util)
- `extend_where_clause` now accepts `IntoIterator` instead of `Vec`
- `whole_type_bound_targets` returns an iterator instead of `Vec`
- Internal visibility tightened: `StructIr` methods and `MementoTraitSpec::new` narrowed
  to `pub(super)`

### Fixed

- Missing `const` on generated const function definitions

## [0.1.0] - 2026-03-01

### Added

- `Recallable`, `Recall`, and `TryRecall` traits with blanket `TryRecall` impl for all
  `Recall` types
- `#[derive(Recallable)]` — generates companion memento struct, exposed as
  `<T as Recallable>::Memento`
- `#[derive(Recall)]` — generates infallible state restoration
- `#[recallable_model]` attribute macro — injects both derives plus optional serde
  integration
- `#[recallable]` field attribute for recursive recalling
- `#[recallable(skip)]` field attribute to exclude fields from memento
- `serde` feature (default) — memento derives `Deserialize`;
  `#[recallable_model]` adds `Serialize` and `#[serde(skip)]`
- `impl_from` feature — generates `From<Struct>` for memento type
- `no_std` compatible
