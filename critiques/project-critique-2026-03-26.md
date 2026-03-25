# Recallable Project Critique — 2026-03-26

## Summary

Recallable is a well-engineered Rust library implementing the Memento pattern via proc macros. The codebase is clean, well-tested, and thoughtfully designed. This report covers what works well, what could be improved, and concrete suggestions.

---

## What Is Good

### Architecture & Design

- **Clean trait hierarchy.** `Recallable` → `Recall` / `TryRecall` with a blanket impl mirrors the stdlib `From`/`TryFrom` pattern. Idiomatic and easy to understand.
- **Two-crate workspace is correct.** Proc-macro crates must be separate; the split is clean with no leaky abstractions.
- **Single IR (`StructIr`) is the right call.** All analysis happens in `StructIr::analyze()`, and codegen modules consume it as read-only. This keeps the data flow unidirectional and easy to reason about.
- **Dependency-closed generic retention** is sophisticated and correct. The fixed-point loop for propagating generic param dependencies through where-clause predicates handles transitive cases that naive approaches miss. Synthetic `PhantomData` markers for unreferenced-but-retained params are a nice touch.
- **`MementoTraitSpec` centralization** keeps derive/bound logic in one place instead of scattering serde conditionals across codegen modules.
- **`no_std` support** from day one is good foresight for embedded/WASM use cases.
- **Memento visibility matching the source struct** is a thoughtful detail that most derive libraries get wrong.

### Code Quality

- Zero clippy warnings with `--all-features`.
- Clean `cargo fmt`.
- Good use of `const fn` where applicable (`StructShape::from_fields`, `FieldStrategy::is_skip`).
- Generated code is wrapped in `const _: () = { ... }` blocks — prevents name leakage.
- `#[inline]` on generated `recall`/`from` methods is appropriate for what are typically small field-assignment functions.
- Doc comments on public APIs include runnable examples that double as compile-time tests.
- The `crate_path()` function correctly handles crate renames and the doctest edge case (`::recallable` instead of `crate`).

### Testing

- **Excellent test coverage strategy.** Four feature combinations (default, no-default, impl_from, all-features) exercised in CI and Makefile.
- **Property-based tests** with proptest across both JSON and postcard backends — this catches edge cases that hand-written tests miss.
- **trybuild UI tests** for 13 compile-fail scenarios covering all error paths.
- **Container impl tests** (`ReplacingOption`, `SelectiveOption`, `ZippedVec`) demonstrate that the macro doesn't prescribe container semantics — a key design decision that's well-validated.
- **Schema drift tests** prove forward/backward compatibility behavior explicitly.
- **Fuzz targets** for both JSON and postcard deserialization+recall paths.
- **100% function coverage threshold** in CI is ambitious and enforced.
- Unit tests in the macro crate itself test IR analysis and codegen in isolation.

### CI & Tooling

- MSRV validation on Rust 1.88.0 as a separate CI job.
- Coverage comparison against base commit with sensible thresholds (100% function, 90% line/region).
- Dependabot for both GitHub Actions and Cargo dependencies.
- Makefile provides convenient local equivalents of CI steps.
- Coverage script extracted to a standalone bash file — clean separation.

### Documentation

- README is thorough: motivation, features, installation, multiple usage examples, how-it-works section, API reference.
- CLAUDE.md is unusually detailed and accurate — a genuine asset for AI-assisted development.
- CONTRIBUTING.md covers the full workflow including fuzzing instructions.
- CHANGELOG follows Keep a Changelog format.

---

## What Could Be Improved

### Architecture & Design

1. **No enum support.** The error message says "This derive macro can only be applied to structs" — fair for v0.1, but enums are a natural next step. Many real-world state types are enums. Worth noting in the README limitations section (it's mentioned but could be more prominent).

2. **`#[recallable_model]` and `#[derive(Recallable)]` have subtly different capabilities** but this isn't well-documented for users. `#[recallable_model]` sees all attributes (it's an attribute macro), while `#[derive(Recallable)]` may not see sibling derives in the same `#[derive(...)]` attribute. This matters for the planned derive-intersection feature and could confuse users who mix the two entry points.

3. **The `Recallable` trait has no method — it's purely a type-level association.** This is fine, but consider whether a convenience method like `fn memento(&self) -> Self::Memento` (requiring `impl_from`) would reduce boilerplate for the common snapshot-then-restore pattern. This could be a separate trait or a provided method gated on `From`.

4. **No `Eq` in `MementoTraitSpec::common_traits`.** Mementos derive `PartialEq` but not `Eq`. For types where all fields are `Eq`, this is a missed opportunity. Users who want `Eq` on their memento currently can't get it without manual impls.

### Code

5. **`is_phantom_data` is a heuristic that could false-positive.** Any user type whose last path segment is `PhantomData` will be treated as phantom. The doc comment acknowledges this, but there's no escape hatch (e.g., `#[recallable(not_phantom)]`). In practice this is unlikely to bite, but it's worth noting.

6. **`FieldBehavior` enum is internal-only but `FieldStrategy` is the one used in `FieldIr`.** The two-step classification (`FieldBehavior` → `FieldStrategy`) adds a layer of indirection. `FieldBehavior::Keep` maps to either `StoreAsSelf` or `StoreAsMemento` depending on whether `#[recallable]` is present. This could be simplified to a single enum if `FieldBehavior` tracked the `#[recallable]` attribute directly.

7. **`collect_recall_like_bounds` and `collect_shared_memento_bounds` naming.** The "recall-like" terminology is not immediately clear. These functions compute where-clause bounds for the `Recallable`/`Recall` impls and the memento struct respectively. More descriptive names like `collect_impl_where_bounds` and `collect_memento_struct_bounds` would help.

8. **`build_recall_param_name` returns `_memento` for empty fields.** This is correct (avoids unused-variable warnings), but the pattern of checking emptiness to decide the parameter name is a bit fragile. A `#[allow(unused_variables)]` attribute on the generated method would be simpler and more robust.

9. **`recallable-macro/Cargo.toml` has `serde = []` as a feature flag with no dependencies.** This is intentional (it's a compile-time `cfg` flag, not a dependency), but it's unusual. A comment in the Cargo.toml would help future contributors understand why.

### Testing

10. **No tests for the `#[recallable_model]` attribute macro's attribute injection behavior in isolation.** The `model_macro.rs` unit tests only cover `is_serde_serialize_path`. The actual `expand()` function is only tested indirectly through integration tests. A unit test that verifies the output token stream contains the expected derives would catch regressions faster.

11. **UI test `.stderr` files are not checked into the glob listing.** They exist (the glob found them), but there's no test verifying the exact error messages haven't drifted. trybuild handles this, but if stderr files get stale, `cargo test` will fail silently on some platforms. Consider running `TRYBUILD=overwrite cargo test` periodically and committing the results.

12. **No test for the `TryRecall` blanket impl's `Infallible` error type.** `basic.rs` tests a custom `TryRecall` impl, and `serde_json.rs` tests the blanket impl succeeds, but no test asserts that the blanket impl's error type is `Infallible`.

13. **`container_impls.rs` has `#[cfg(not(feature = "impl_from"))]` on `SelectiveOptionOuter`.** This means the struct is only compiled without `impl_from`. The reason (likely: `SelectiveOption` doesn't impl `From`) should be documented with a comment explaining why, or better, a compile-fail UI test proving it.

14. **Fuzz targets don't assert post-recall invariants.** `apply_memento` calls `recall` but doesn't check that the skipped field is preserved or that recalled fields match. Adding assertions would turn the fuzzer into a property-based oracle, not just a panic detector.

### Documentation

15. **README "How It Works" section could show the generated code.** Users of derive macros benefit enormously from seeing what the macro actually produces. A `cargo expand` snippet for a simple struct would demystify the library.

16. **No `docs.rs` metadata in `Cargo.toml`.** Adding `[package.metadata.docs.rs]` with `all-features = true` ensures docs.rs builds with serde and impl_from enabled, showing the complete API.

17. **CHANGELOG only has 0.1.0.** This is fine for now, but the recent refactoring commits (MementoTraitSpec centralization, visibility matching, CodegenEnv simplification) represent meaningful changes that should be captured before the next release.

### CI & Tooling

18. **Coverage comparison rebuilds the entire project from the base SHA.** This is expensive — the `coverage-compare` job checks out the base commit and runs the full test matrix again. For a small project this is fine, but as the project grows, consider caching base coverage or using a coverage-diff tool.

19. **No `cargo doc --no-deps` check in CI.** Broken doc links or missing doc examples won't be caught until someone runs `cargo doc` locally or publishes to docs.rs.

20. **No `cargo test --doc` as a separate CI step.** Doc-tests run as part of `cargo test`, but calling them out explicitly makes failures easier to diagnose.

21. **The `cpu-profiling` Cargo profile exists but isn't referenced anywhere.** If it's for future use, a comment would help. If it's unused, remove it.

---

## Prioritized Suggestions

### High Value, Low Effort

- Add `[package.metadata.docs.rs]` with `all-features = true` to both Cargo.toml files
- Add `cargo doc --no-deps --all-features` to CI
- Add a comment in `recallable-macro/Cargo.toml` explaining why `serde = []` has no deps
- Document the `#[cfg(not(feature = "impl_from"))]` guard in `container_impls.rs`

### High Value, Medium Effort

- Split `context.rs` into submodules (generics, fields, lifetime utilities)
- Add a "Generated Code" section to the README with `cargo expand` output
- Add post-recall assertions to fuzz targets
- Add a unit test for `model_macro::expand()` output

### Medium Value, Future Consideration

- Enum support
- `Eq` in common traits (or the planned derive-intersection feature)
- Convenience `memento()` method on `Recallable`
- Escape hatch for `is_phantom_data` heuristic

---

## Overall Assessment

This is a high-quality Rust library. The architecture is sound, the testing is thorough (property tests, fuzz targets, UI tests, multi-feature matrix), and the documentation is above average for a proc-macro crate. The main area for improvement is the size of `context.rs` and some minor documentation gaps. The codebase is well-positioned for the features being planned (derive-intersection, etc.).
