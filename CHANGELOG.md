# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-27

### Added

- Comprehensive `GUIDE.md` for detailed library documentation

- "Generated Code" section in README showing `cargo expand` output

- `#[recallable(skip_memento_default_derives)]` struct-level attribute to suppress default
  `Clone`/`Debug`/`PartialEq` derives on the generated memento

### Changed

- Memento type visibility now matches the source struct (e.g. `pub(crate) struct` ->
  `pub(crate) struct Memento`)

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

[0.2.0]: https://github.com/ShapelessCat/recallable/releases/tag/v0.2.0
[0.1.0]: https://github.com/ShapelessCat/recallable/releases/tag/v0.1.0
