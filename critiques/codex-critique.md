# Codex Critique — 2026-03-26

## Overall Assessment

This is a strong v0.1 Rust project. The core design is thoughtful, the proc-macro architecture is better than most small macro crates, and the testing / CI discipline is already above average.

The main weaknesses are not obvious correctness bugs. They are mostly about ergonomics and maintainability: documentation drift, hidden semantic differences between user-facing entry points, and a macro-analysis core that is starting to get too large.

## What Is Good

### Architecture

- The two-crate split is correct. `recallable` owns the traits and public API, while `recallable-macro` owns code generation.
- The trait hierarchy is clean and idiomatic: `Recallable` declares the associated memento type, `Recall` handles infallible application, and `TryRecall` adds the fallible path on top of that in `recallable/src/lib.rs:222`, `recallable/src/lib.rs:267`, and `recallable/src/lib.rs:336`.
- The macro crate uses a real IR instead of ad-hoc token rewriting. `StructIr::analyze` feeding narrower emitters is a good design in `recallable-macro/src/context.rs:1` and `recallable-macro/src/context.rs:221`.
- Hiding the generated companion type behind `<T as Recallable>::Memento` instead of leaking a top-level generated symbol is a strong API choice in `recallable-macro/src/lib.rs:98`.
- Generic pruning plus synthetic marker injection is thoughtful and solves a genuinely tricky proc-macro problem in `recallable-macro/src/context.rs:754` and `recallable-macro/src/context.rs:776`.

### Quality And Testing

- Test strategy is genuinely strong for a new crate: integration tests, `trybuild` UI failures, property tests, fuzz targets, and feature-matrix coverage.
- CI quality bars are serious: format, clippy, MSRV, coverage reporting, and coverage comparison are all wired up in `.github/workflows/ci.yaml:14` and `.github/workflows/ci.yaml:105`.
- Documentation quality is above average. `README.md`, crate docs, examples, `CONTRIBUTING.md`, and `CLAUDE.md` all have useful substance instead of placeholder text.
- The project’s central design idea — container semantics are type-defined, not hard-coded by the macro — is clearly exercised in `recallable/tests/container_impls.rs:1`.

## What Is Bad / Risky

### 1. Documentation Drift On Lifetime Support

- `README.md:116` says lifetime-parameterized structs are rejected outright.
- The tests show supported lifetime cases such as pure `PhantomData` and skipped borrowed fields in `recallable/tests/basic.rs:71` and `recallable/tests/basic.rs:95`.
- This is not a tiny wording issue. It gives users the wrong model of what the crate actually supports.

### 2. CI And Contributor Guidance Do Not Validate The Whole Workspace

- Stable and MSRV jobs mainly run `cargo test --package recallable` in `.github/workflows/ci.yaml:37` and `.github/workflows/ci.yaml:78`.
- Contributors are told to do the same in `CONTRIBUTING.md:27`.
- But `recallable-macro` has real unit tests in `recallable-macro/src/context.rs:1059` and `recallable-macro/src/model_macro.rs:83`.
- Result: the macro crate is under-validated in the normal development path.

### 3. The Two User-Facing Entry Points Have Different Wire-Format Semantics

- `#[recallable_model]` injects `serde::Serialize` and `#[serde(skip)]` in `recallable/src/lib.rs:25` and `recallable-macro/src/model_macro.rs:16`.
- Plain `#[derive(Recallable, Recall)]` does not.
- The difference is real and tested in `recallable/tests/serde_json.rs:109`, `recallable/tests/serde_json.rs:126`, and `recallable/tests/postcard.rs:133`.
- This is a footgun: swapping macros can change serialized shape without changing the Rust fields.

### 4. Important Macro Limitations Are Still Mostly Learned Through Compile Failure

- `#[recallable]` only accepts path-shaped field types in `README.md:118` and `recallable-macro/src/context.rs:667`.
- Non-skipped borrowed fields are rejected in `recallable-macro/src/context.rs:532`.
- Struct-only support is enforced in `recallable-macro/src/context.rs:521`.
- The diagnostics are decent, but users still learn too much by tripping over the macro instead of through docs.

### 5. `impl_from` Is Useful, But It Leaks Into Model Design

- The feature only generates consuming `From<Struct>` in `recallable-macro/src/lib.rs:74`.
- Some otherwise reasonable container memento shapes do not fit it; `SelectiveOptionOuter` is disabled under `impl_from` in `recallable/tests/container_impls.rs:70`.
- That makes the feature feel more opinionated than the base crate design.

### 6. Generated Mementos Are Probably Over-Constrained

- Every generated memento hard-codes `Clone`, `Debug`, and `PartialEq`, plus optional `Deserialize`, in `recallable-macro/src/context.rs:145` and `recallable-macro/src/lib.rs:70`.
- That is understandable and practical, but it is stronger than the conceptual core of `Recallable` / `Recall`.
- It reduces the set of field types that can participate in derives and makes the crate feel more policy-heavy than it first appears.

### 7. The Macro Crate Has A Real Maintenance Hotspot

- `recallable-macro/src/context.rs` is 1377 lines and mixes IR types, field analysis, generic planning, bound synthesis, heuristics, diagnostics, and unit tests.
- The emitter submodule split is good, but the analysis core is still a monolith.
- The bound synthesis flow is also a bit brittle because it builds one ordered predicate list and later splits it by count in `recallable-macro/src/context.rs:426` and `recallable-macro/src/context.rs:451`.

### 8. Fuzzing Only Checks Panic Safety, Not Semantic Correctness

- The fuzz helper just applies the memento and stops in `fuzz/fuzz_targets/common.rs:43`.
- That catches panics, but not logic regressions such as skipped fields being overwritten or nested recall producing the wrong state.

### 9. Project Automation Has A Small Blind Spot

- Dependabot watches Cargo in `/`, but not the separate fuzz crate in `/fuzz`, as shown in `.github/dependabot.yaml:7` and `fuzz/Cargo.toml:1`.

### 10. `TryRecall::Error` May Be Stricter Than Necessary

- Requiring `core::error::Error + Send + Sync + 'static` in `recallable/src/lib.rs:338` rules out some lighter `no_std`-friendly error designs.
- This may be intentional, but the docs do not explain why the extra restriction is worth it.

## Advice For Improvement

### Highest-Value Short-Term Fixes

1. Make workspace validation the default.
   Use `cargo test --workspace` in CI and `CONTRIBUTING.md`, or explicitly add `-p recallable-macro`.
2. Fix the lifetime-support docs immediately.
   Align `README.md` with actual behavior and existing tests.
3. Add a “macro behavior matrix”.
   Compare `#[recallable_model]` vs `#[derive(Recallable, Recall)]` across `serde` and `impl_from`.
4. Promote one container-semantics example into `examples/`.
   Replacement vs selective-update would make the project’s key design idea much easier to grasp.
5. Add Dependabot coverage for `/fuzz`.
6. Add `package.metadata.docs.rs` and build docs with the intended feature set.

### Medium-Term Engineering Improvements

1. Replace position-dependent bound assembly with structured buckets.
   That will make refactors safer and the code easier to reason about.
2. Add more targeted macro tests.
   Good candidates: crate rename resolution, duplicate derive edge cases, attribute-order footguns, and repeated `#[recallable]` attributes.
3. Upgrade the fuzz harness from panic-only to invariant-checking.
   Assert skipped-field preservation and expected nested recall behavior.

### Longer-Term Product / API Considerations

1. Revisit whether hard-coded memento derives should remain fixed.
   A user opt-in extension like `memento_derive(...)` would be more flexible.
2. Consider a non-consuming snapshot path.
   `From<&T>` or a `snapshot(&self)` convenience would reduce friction.
3. Either relax `TryRecall::Error` bounds or document the rationale clearly.
4. Improve diagnostics for unsupported non-path `#[recallable]` types so the compiler error explains the supported shapes.

## Bottom Line

This is a good project. The hard parts are mostly already done correctly: the conceptual API is clean, the proc-macro architecture is real rather than hacky, and the test discipline is stronger than many crates at this stage.

The main problem is not “bad Rust code.” It is that the project is slightly easier for the maintainer to understand than for the user to adopt. If you tighten the docs, make the entry-point differences more explicit, and pay down the `context.rs` maintainability debt, the crate will feel much more polished.

## Validation Snapshot

The following commands passed during review:

- `cargo test -p recallable`
- `cargo test -p recallable --no-default-features`
- `cargo test -p recallable --features impl_from`
- `cargo test -p recallable --all-features`
- `cargo test -p recallable-macro`
- `cargo test -p recallable-macro --all-features`
