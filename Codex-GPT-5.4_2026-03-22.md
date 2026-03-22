# Project Critique: recallable

## Scope

This report was written before reading
`Gemini-3.1-Pro-High_antigravity_1.0_2026-03-22.md`.

The review is based on:

- Repository source and docs
- Local test runs
- Local `clippy` run
- Small offline throwaway crates used to confirm edge-case behavior

## Bottom Line

This project is more disciplined than many early Rust macro crates. The test matrix is real,
`clippy` is clean, and the core idea is narrow enough to stay understandable.

The main problem is not code rot. The main problem is product definition. The crate presents
itself as a general solution for state snapshots and recursive recalling, but the actual macro
surface is still narrow, opinionated, and full of implicit constraints. That mismatch will hurt
users faster than any current implementation bug.

## What Is Good

- The workspace is small and focused. There is not much accidental complexity.
- CI is stronger than expected for a `0.1.0` crate. The project checks stable, MSRV,
  multiple feature combinations, and coverage in `.github/workflows/ci.yaml`.
- The tests cover happy paths, `serde_json`, `postcard`, `no_std`-leaning usage, `trybuild`
  failure cases, and `impl_from`.
- The public trait surface in `recallable/src/lib.rs` is small and readable.
- The proc-macro implementation is structured instead of being one large expansion function.

## Main Criticisms

### 1. The README oversells what the macros can actually do

The strongest marketing claim in the README is also one of the least defensible ones.
`README.md:60` says there is "Full support for generic types", but `README.md:239` says
`#[recallable]` only supports simple generic types and not types like `Vec<T>`.

That is not a wording nit. It is a product-positioning problem.

- Container types such as `Option<T>` and `Vec<T>` are rejected for `#[recallable]`.
- Associated types such as `<T as Trait>::Inner` are rejected.
- Any lifetime parameter is rejected.

I confirmed two of those limits with throwaway offline crates:

- A marker-only lifetime like `PhantomData<&'a ()>` still fails with
  `Recall derives do not support borrowed fields`.
- A field marked `#[recallable]` with type `<T as HasInner>::Inner` fails with
  `Only a simple generic type is supported here`.

For a proc-macro crate, expectation management is part of correctness. Right now the README
creates expectations the implementation cannot meet.

### 2. The generated memento types impose hidden trait requirements

The derive path quietly requires more from user types than the public traits suggest.

In `recallable-macro/src/context/memento_struct.rs:15-19`, generated memento types always derive:

- `Clone`
- `Debug`
- `PartialEq`
- `Deserialize` when the `serde` feature is enabled

That means a perfectly reasonable state field can fail to derive `Recallable` if it does not
implement those traits, even when the user only wants memento generation and recall behavior.

I confirmed this with an offline throwaway crate: a plain field type without `Clone`, `Debug`,
and `PartialEq` fails during derive expansion because the generated memento struct wants all
three.

This is a bad kind of magic. The API looks minimal:

```rust
pub trait Recallable {
    type Memento;
}
```

But the derive macro effectively adds a bigger, undocumented contract on top.

### 3. Lifetime handling is broader than the docs imply and the error message is misleading

`recallable-macro/src/context.rs:90-104` rejects any lifetime generic at all. The check is on
generic parameters, not on borrowed fields.

That matters because the docs repeatedly frame the limitation as "borrowed fields" in:

- `README.md:238`
- `README.md:291-302`
- The compiler error text in `recallable-macro/src/context.rs:97-100`

Those are not the same thing.

A type with a lifetime marker but no borrowed runtime data is still rejected. That makes the
error message misleading and hides a more fundamental limitation: the derive macros do not support
any lifetime-parameterized struct shape, even harmless ones.

### 4. `#[recallable_model]` is too opinionated about `serde`

The attribute macro does more than a typical convenience macro should.

In `recallable-macro/src/lib.rs:38-69`, `#[recallable_model]`:

- Adds `Recallable` and `Recall`
- Adds `serde::Serialize` when the feature is enabled
- Injects `#[serde(skip)]` into fields marked `#[recallable(skip)]`
- Rejects an existing `Serialize` derive in `recallable-macro/src/lib.rs:159-180`

That is convenient for the author's preferred workflow, but it reduces composability.

Problems this creates:

- Users lose control over whether serialization derives are explicit or implicit.
- The macro becomes harder to reason about because it mutates unrelated attributes.
- The crate couples "state recalling model" with "this exact serde behavior" more tightly than
  necessary.

For a macro library, invisible attribute mutation should be treated as expensive API surface, not
free convenience.

### 5. Onboarding is weaker than the implementation quality

The installation story and examples are not aligned.

- `README.md:72-75` tells users to add only `recallable`.
- The first example in `README.md:83-113` also needs `serde` and `postcard`.
- The Features section has visible wording problems, including
  `README.md:52-53`, which reads like an unfinished edit.

This is small in isolation, but first impressions matter more for macro crates than for regular
libraries. Users copy examples before they read limitations.

The contributing docs also hint at missing example material. `CONTRIBUTING.md:33-39` contains a
commented-out "Running Examples" section with a TODO, which suggests the project knows examples
are missing but has not closed that loop.

### 6. The project shows "maintainer thoroughness" more than "user empathy"

The repo has signs of strong maintainer discipline:

- CI matrix
- coverage comparison
- MSRV validation
- `trybuild` failure tests

But several user-facing assets feel thin:

- The changelog is still basically a one-entry bootstrap note in `CHANGELOG.md:8-13`.
- There is no dedicated examples directory even though the crate is macro-heavy.
- Editor-specific config is committed in `.vscode/settings.json:1-6`.

The result is a project that looks careful internally but not yet polished externally.

That is common at `0.1.0`, but it is still a valid criticism. Macro crates compete heavily on
clarity, predictability, and examples, not just correctness.

## Evidence From Local Validation

The current codebase is not sloppy. It passed the checks I ran:

- `cargo test --package recallable`
- `cargo test --package recallable --no-default-features`
- `cargo test --package recallable --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`

So the critique is not "this project is broken". The critique is:

1. The project promise is broader than the implementation.
2. The macro behavior is more magical than the public API suggests.
3. The user experience is under-documented in exactly the places where proc-macro users need
   precision.

## Priority Fixes

### High Priority

- Rewrite the README claims around generic support so they match real macro behavior.
- Document all implicit trait requirements introduced by generated mementos.
- Change the lifetime-related wording from "borrowed fields are unsupported" to
  "lifetime-parameterized structs are unsupported", unless the implementation is expanded.
- Make `#[recallable_model]` less magical, or at least document every injected attribute
  prominently.

### Medium Priority

- Add a real `examples/` directory with one minimal example and one nested-generic example.
- Add a dedicated "limitations and non-goals" section near the top of the README, not near the
  bottom.
- Decide whether `.vscode/settings.json` is intentionally project-wide; if not, stop tracking it.

### Low Priority

- Clean up wording issues in the README.
- Expand the changelog beyond the initial release note.
- Reduce the gap between the sophistication of CI and the sophistication of user-facing docs.

## Final Assessment

This is a promising crate with solid engineering habits and a still-immature user contract.

If I were deciding whether to depend on it today, my answer would be:

- Yes for controlled internal use where I can accept the macro limitations.
- Not yet for broad library-facing adoption, because the docs and macro contract are still too
  narrow, too magical, and too easy to misread.

## Comparison Summary Against Gemini Report

After reading `Gemini-3.1-Pro-High_antigravity_1.0_2026-03-22.md`, the overlap is real:

- We both think the project is structurally clean, well tested, and more disciplined than most
  early macro crates.
- We both call out the narrow support for complex generic shapes and the hardcoded derives on
  generated mementos.
- We both see the crate as promising, but still constrained.

Gemini emphasizes feature expansion:

- Better support for container types like `Vec<T>` and `Option<T>`
- Possible enum support
- User-configurable derives or attributes on generated mementos

I agree those are useful directions, but I would rank the current problems differently.

My stronger conclusion is that the project's first risk is not missing capability. The first risk
is an imprecise user contract.

- The README markets broader generic support than the implementation provides.
- The lifetime limitation is documented less precisely than it is implemented.
- `#[recallable_model]` hides important `serde` behavior behind attribute mutation.
- The installation and example story is still weaker than the engineering underneath it.

So my comparison takeaway is:

1. Gemini is right about the next feature opportunities.
2. I think the more urgent fix is to tighten the docs and make the macro contract explicit.
3. Expanding capability before clarifying behavior would make the crate more powerful, but not
   necessarily more trustworthy.
