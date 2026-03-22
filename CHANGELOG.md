# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
