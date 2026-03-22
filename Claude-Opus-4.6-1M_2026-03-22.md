# Project Critique: recallable

**Date:** 2026-03-22
**Evaluator:** Claude Opus 4.6 (1M context)

## Executive Summary

`recallable` is a focused Rust proc-macro crate implementing the Memento design
pattern. It generates companion "memento" structs from annotated source structs
and wires up state-restoration logic at compile time. The project shows strong
internal engineering discipline: clean clippy, comprehensive test matrix, coverage
tracking, and a well-factored macro implementation. However, the crate suffers
from a gap between what it promises and what it can deliver, several implicit
contracts that are not surfaced to users, and rough edges in documentation and
project scaffolding that would make adoption harder than the code quality warrants.

---

## 1. Architecture and Trait Design

### Strengths

- **Clean three-trait hierarchy.** `Recallable` (declares the memento type),
  `Recall` (infallible application), and `TryRecall` (fallible application with
  custom error) form a well-separated API. The blanket `TryRecall` impl for all
  `Recall` types avoids duplication and matches Rust idioms (`TryFrom`/`From`).

- **Hidden memento struct.** The generated companion type lives inside
  `const _: () = { ... }` and is only accessible as `<Struct as Recallable>::Memento`.
  This prevents namespace pollution and gives the macro freedom to change the
  internal struct name without breaking downstream code.

- **`no_std` by default.** The traits crate is `#![no_std]`, which is the right
  default for a library of this kind. The use of `core::error::Error` for the
  `TryRecall` bound (stabilized in recent editions) is a smart forward-looking
  choice.

- **Well-structured macro internals.** The `MacroContext` pattern, with separate
  modules for memento struct generation, `Recallable` impl, `Recall` impl,
  `From` impl, and utility helpers, is clean and maintainable. Each module has
  a single responsibility.

### Weaknesses

- **`TryRecall` cannot be derived.** `Recall` (infallible) has a derive macro,
  but `TryRecall` does not. Users who need validation logic during recall must
  implement the full trait manually, including re-stating the `Recallable`
  relationship. For a crate that markets itself on reducing boilerplate, this is
  a notable asymmetry. A derive macro for `TryRecall` (or at least a helper that
  generates the skeleton with a user-provided validation closure) would close
  this gap.

- **No mechanism for partial recall.** The `recall` method replaces all
  non-skipped fields unconditionally. There is no way to express "apply only the
  fields that are present" (like serde's `Option<T>` flattening pattern). This
  limits the crate's applicability to true partial-update scenarios, which are
  exactly what the README motivates the project with.

- **`Recallable` and `Recall` must be derived together in practice.** Although
  they are separate derives, `Recall` requires `Recallable` to already be
  implemented (for `Self::Memento`). The separation is conceptually clean but
  operationally fragile: if a user forgets to derive one, they get an error about
  the missing `Memento` type that does not clearly point to the missing derive.
  `#[recallable_model]` papers over this, but users who use the derives directly
  are left to figure it out.

---

## 2. Macro Surface and Implicit Contracts

### 2.1. Hardcoded Derives on Generated Memento

In `memento_struct.rs:15-18`, the generated memento always derives:

- `Clone`
- `Debug`
- `PartialEq`
- `Deserialize` (when `serde` feature is enabled)

This is invisible to the user but imposes hard requirements. If any field type
in the original struct does not implement `Clone`, `Debug`, or `PartialEq`, the
derive expansion fails with an error that points at the generated code, not at
the user's struct. These requirements are not documented in the README, the API
reference, or the doc comments on the derive macros.

Worse, there is no mechanism for users to customize what the memento derives.
A user who needs `Eq`, `Hash`, `Default`, or `Ord` on the memento cannot get it
without manually implementing `Recallable` and defining the memento struct
themselves, defeating the purpose of the macro.

**Recommendation:** Add a `#[recallable(memento_derive(Eq, Hash, ...))]` or
similar attribute to let users control the generated derive list, or at minimum
document the implicit requirements prominently.

### 2.2. Lifetime Rejection Is Broader Than Documented

The `validate_generics` function in `context.rs:90-104` rejects any struct with
a lifetime parameter:

```rust
if input.generics.params.iter().any(|g| matches!(g, GenericParam::Lifetime(_)))
```

But the error message says "Recall derives do not support borrowed fields." This
is misleading. The check rejects *any lifetime parameter on the struct*, not just
borrowed fields. A struct like:

```rust
struct Foo<'a> {
    data: PhantomData<&'a ()>,
}
```

contains no borrowed runtime data but is still rejected. The documentation in
`README.md:238` and `README.md:291` also frames this as "borrowed fields" or
"lifetime parameters (borrowed fields)," conflating two different things.

**Recommendation:** Change the error message to "Recall derives do not support
lifetime-parameterized structs" and update the README to match, unless the
implementation is broadened to only reject actual borrow types.

### 2.3. `#[recallable]` Field Attribute Has a Narrow Definition of "Simple"

The `extract_recallable_type_name` function in `context.rs:201-227` only accepts
a bare single-segment path with no generic arguments. This means:

- `T` is accepted (bare type parameter).
- `Option<T>` is rejected ("Only a simple generic type is supported here").
- `Vec<T>` is rejected.
- `<T as Trait>::Assoc` is rejected.
- `mod::ConcreteType` is accepted but does not track generic params.

The README line 60 claims "Full support for generic types," which directly
contradicts these limits. The Limitations section at line 239 partially corrects
this, but the feature bullet at line 60 is what users will read first.

**Recommendation:** Change the feature claim to something like "Support for
simple generic type parameters" and move the limitations section closer to the
top of the README.

---

## 3. `#[recallable_model]` and Serde Coupling

### 3.1. Implicit Attribute Mutation

`#[recallable_model]` does more than inject derives. When the `serde` feature is
enabled, it:

1. Adds `#[derive(serde::Serialize)]` to the struct.
2. Injects `#[serde(skip)]` onto fields marked `#[recallable(skip)]`.
3. Rejects any pre-existing `#[derive(Serialize)]` with a compile error.

This means the macro silently mutates attributes that are not its own. While
convenient in the common case, it creates a coupling between the state-management
concern (`recallable`) and the serialization concern (`serde`) that users cannot
opt out of when using `#[recallable_model]`.

For instance, a user who wants `Serialize` with custom `#[serde(...)]` container
attributes (like `#[serde(rename_all = "camelCase")]`) must be careful about
ordering: `#[recallable_model]` must appear first in source order or it will not
see the other attributes. This ordering requirement exists because attribute
macros only see attributes that appear *after* them in source order, but it is
a subtle footgun that is only documented in `CLAUDE.md` and the project's
internal memory, not in user-facing docs.

### 3.2. Asymmetric Serialization Design

The generated memento derives `Deserialize` but not `Serialize`. The README
states this is "by design" but does not explain the rationale. From reading the
code, the intent is that the *original struct* is serialized and the *memento* is
deserialized from that serialized form. This only works when the memento's field
layout exactly mirrors the original struct's serialized form minus skipped fields.

This creates an implicit structural coupling: the memento's field ordering,
naming, and types must be wire-compatible with the original struct's serialized
representation. If a user adds `#[serde(rename = "...")]` to the original struct
but cannot apply the same rename to the invisible memento, deserialization will
silently fail at runtime.

**Recommendation:** Either document this structural coupling explicitly, or
provide a way to forward serde attributes from the original struct's fields to
the corresponding memento fields.

---

## 4. Testing

### Testing Strengths

- **Good breadth.** The test suite covers `serde_json`, `postcard`, `no_serde`,
  `impl_from`, named structs, tuple structs, unit structs, generic types, nested
  recallable fields, skipped fields, where clauses, and compile-fail UI tests.
  This is impressive for a `0.1.0` crate.

- **`trybuild` for error messages.** Compile-fail tests with expected stderr
  snapshots ensure error messages do not regress, which is critical for
  proc-macro crates.

- **Separate feature-gated test binaries.** Each test file has explicit
  `required-features` in `Cargo.toml`, preventing accidental feature leakage.

### Testing Weaknesses

- **No negative runtime tests for edge cases.** The tests verify that correct
  usage works and that certain macro invocations fail at compile time, but there
  are no tests for runtime edge cases like: What happens when a memento is
  deserialized from malformed data? What happens when fields are reordered
  between serialization and deserialization? These are the scenarios most likely
  to bite users of a persistence-focused library.

- **Doc tests are minimal.** Only two doc tests exist (on `Recallable` and
  `TryRecall`). The `Recall` trait, the `recallable_model` macro, and the
  field-level attributes have no doc tests. For a macro-heavy crate, executable
  documentation is especially valuable.

- **No property-based or fuzz testing.** For a crate that generates code handling
  serialization boundaries, property-based tests (e.g., with `proptest` or
  `quickcheck`) verifying round-trip invariants would add significant confidence.

---

## 5. Documentation and Onboarding

### 5.1. README Issues

- **Line 52-53** reads: "More or creating a derive attribute, inserting
  `Recallable` and `Recall`..." This is an incomplete or garbled sentence that
  looks like an unfinished edit.

- **Line 60** claims "Full support for generic types" but line 239 says
  `#[recallable]` only supports simple generics. This is a direct contradiction
  within the same document.

- **The installation section** (lines 72-77) tells users to add only `recallable`
  to their dependencies, but the first example (lines 83-113) immediately uses
  `serde`, `postcard`, and `heapless` without mentioning they need to be added
  separately.

- **The Limitations section** is at the bottom of the README (line 235+), after
  all the examples. Users will have already copy-pasted code and hit confusing
  errors before discovering the constraints. For a proc-macro crate, limitations
  should appear near the top.

### 5.2. Missing Examples Directory

There is no `examples/` directory. `CONTRIBUTING.md` line 33-39 contains a
commented-out "Running Examples" section with a TODO, acknowledging this gap.
For a proc-macro crate, standalone runnable examples are one of the most
effective onboarding tools.

### 5.3. Changelog

The changelog contains a single bootstrap entry. While expected at `0.1.0`, it
does not describe the initial feature set in enough detail to be useful for
evaluation.

---

## 6. Project Hygiene

### 6.1. Editor Configuration Committed

`.vscode/settings.json` is tracked in git. While it only contains file
associations for the license files, committing editor-specific configuration
is generally bad practice. It should be in `.gitignore` or a
`.vscode/settings.json.example`.

### 6.2. `.gitignore` Misses `.vscode/`

The `.gitignore` excludes `.idea/` (IntelliJ) but not `.vscode/`. This is
inconsistent: either both should be ignored, or both should be committed. The
current state suggests the `.vscode` inclusion is accidental rather than
intentional.

### 6.3. Dependabot Configuration

The `dependabot.yaml` only covers `github-actions` updates, not Cargo
dependencies. For a crate with pinned `proc-macro2`, `syn`, `quote`, and
`proc-macro-crate` versions, automated dependency update PRs would be
valuable.

### 6.4. CI Coverage Thresholds

The coverage comparison job is impressively thorough (function >= 100%, line
>= 90%, region >= 90%), but the thresholds are implemented entirely in shell
script within the CI YAML. This is hard to maintain and test locally. Extracting
the coverage logic into a script or using a dedicated action would improve
maintainability.

---

## 7. Code-Level Observations

### 7.1. `HashMap` for `preserved_types` Uses Non-Deterministic Iteration

`MacroContext` uses `HashMap<&Ident, TypeUsage>` for `preserved_types`. Since
`HashMap` iteration order is non-deterministic, the order of generic parameters
in the generated memento struct could theoretically vary between compilations
(though in practice `build_memento_struct_type` iterates `generics.type_params()`
in declaration order and only checks membership). This is currently safe but
fragile if the iteration pattern ever changes.

### 7.2. `SimpleTypeCollector` Over-Collects

The `collect_used_simple_types` visitor in `context.rs:377-394` walks the entire
type tree and collects the first segment of every `TypePath`. For a type like
`std::collections::HashMap<K, V>`, it would collect `std`, `K`, and `V`. The
`std` entry is harmless because it will not match any generic parameter, but the
over-collection is semantically sloppy.

### 7.3. `recall` Methods Are `#[inline(always)]`

Both the generated `recall` method and the blanket `TryRecall::try_recall` are
marked `#[inline(always)]`. For small structs this is fine, but for structs with
many fields, forced inlining could cause code bloat. `#[inline]` (without
`always`) would let the compiler decide.

---

## 8. Recommendations (Prioritized)

### High Priority

1. Fix the README contradiction: replace "Full support for generic types" with
   accurate language and move the Limitations section before the examples.
2. Document the implicit `Clone`, `Debug`, and `PartialEq` requirements imposed
   by the generated memento struct.
3. Fix the lifetime error message to say "lifetime-parameterized structs" instead
   of "borrowed fields."
4. Fix the garbled sentence at README line 52-53.

### Medium Priority

1. Add a `#[recallable(memento_derive(...))]` attribute for user-controlled
   memento derives.
2. Create an `examples/` directory with at least two examples: a minimal one and
   a nested-recallable-with-serde one.
3. Document the `#[recallable_model]` attribute-ordering requirement in
   user-facing docs, not just in internal project files.
4. Add `cargo` to the dependabot configuration.
5. Stop tracking `.vscode/settings.json` or add `.vscode/` to `.gitignore`.

### Low Priority

1. Consider `#[inline]` instead of `#[inline(always)]` for generated methods.
2. Add property-based round-trip tests for serialization invariants.
3. Expand doc tests to cover `Recall`, `recallable_model`, and field attributes.
4. Consider supporting `Option<T>` fields for partial-update semantics.

---

## Final Assessment

`recallable` is a well-engineered `0.1.0` crate with a focused problem domain
and strong internal quality practices. The macro implementation is clean, the
test coverage is broad, and the CI pipeline is more mature than most crates at
this stage.

The core risk is not code quality. The core risk is that the crate's external
contract (README, error messages, implicit requirements) does not accurately
represent what the macros can and cannot do. Users will hit invisible trait
requirements, misleading error messages, and documented features that do not
match reality. These are fixable problems, and fixing them would make the crate
ready for broader adoption.

For controlled internal use where the team understands the constraints, this
crate is ready today. For public library adoption, the documentation and user
contract need tightening first.

---

## Comparison Summary Against Gemini and Codex Reports

After reading `Gemini-3.1-Pro-High_antigravity_1.0_2026-03-22.md` and
`Codex-GPT-5.4_2026-03-22.md`, here is how the three critiques relate.

### Points of Agreement (All Three Reports)

All three reviews converge on the same core observations:

- **The project is well-engineered internally.** Clean code, modular macro
  structure, strong CI, broad test coverage. None of us found implementation
  bugs.
- **Hardcoded memento derives are a problem.** The invisible `Clone`, `Debug`,
  `PartialEq` (and `Deserialize`) requirements on generated mementos surface
  in all three reports as a key limitation.
- **Generic support is narrower than advertised.** All three flag the
  contradiction between the README's "Full support for generic types" claim and
  the actual restriction to bare single-segment type parameters.
- **Lifetime handling is over-broad and mislabeled.** Gemini notes it as a
  "constraint," Codex identifies the precise error message mismatch ("borrowed
  fields" vs. "lifetime-parameterized structs"), and I independently reached the
  same conclusion with the same recommendation.
- **No `examples/` directory.** All three note the commented-out TODO in
  `CONTRIBUTING.md` and the absence of runnable examples.
- **The crate is promising but not yet ready for broad adoption.**

### Where Gemini Differs

Gemini (Antigravity 1.0) emphasizes **feature expansion** as the primary path
forward:

- Recursive `#[recallable]` support for container types (`Vec<T>`, `Option<T>`).
- Enum support for state-machine patterns.
- User-configurable memento derives via `#[recallable(memento_attr(...))]`.

Gemini treats the documentation gaps as secondary to the capability gaps. I
disagree with this prioritization. Expanding capability before clarifying the
existing contract would make the crate more powerful but not more trustworthy.
Users who cannot predict what the current macros do will not be helped by the
macros doing more things.

Gemini also notes the "smart" decision to derive `Deserialize` but not
`Serialize` on mementos. I agree this is intentional, but I flag the structural
coupling risk that Gemini does not: if the original struct has `#[serde(rename)]`
or `#[serde(flatten)]` attributes, the memento's deserialization may silently
break because those attributes are not forwarded to the generated struct.

### Where Codex Differs

Codex (GPT-5.4) is the sharpest of the three on **user contract precision**.
Its critique of `#[recallable_model]` being "too opinionated about serde"
(Section 4 of the Codex report) goes beyond what Gemini or I covered. Codex
specifically calls out:

- The macro mutates attributes it does not own (injecting `#[serde(skip)]`).
- It rejects pre-existing `Serialize` derives, reducing composability.
- The coupling between "state recalling model" and "this exact serde behavior"
  is tighter than necessary.

I agree with this analysis. My report touches on the serde coupling but focuses
more on the attribute-ordering footgun. Codex frames the problem more
fundamentally: the macro does too much invisible work.

Codex also independently confirmed edge cases with offline throwaway crates
(PhantomData lifetimes, associated types), which adds empirical weight to claims
that all three reports make from code reading alone.

Codex uniquely identifies the **onboarding weakness**: the installation section
tells users to add only `recallable`, but the first example needs `serde` and
`postcard`. I caught the same issue but Codex frames it more crisply as a
"first impressions" problem for macro crates.

### What My Report Adds

Several observations in my report are not present in either Gemini or Codex:

1. **No `TryRecall` derive.** Neither report notes the asymmetry between
   `Recall` (has a derive) and `TryRecall` (must be implemented manually). For
   a crate that markets itself on reducing boilerplate, this is a gap worth
   flagging.

2. **No partial-recall semantics.** Neither report observes that the `recall`
   method replaces all non-skipped fields unconditionally, which limits the
   crate's applicability to the "partial update" scenarios the README motivates.

3. **`HashMap` non-determinism risk in `preserved_types`.** A code-level
   observation about potential iteration-order fragility that neither report
   mentions.

4. **`SimpleTypeCollector` over-collection.** The type visitor collects
   irrelevant path segments (like `std` from `std::collections::HashMap`),
   which is harmless but semantically sloppy.

5. **`#[inline(always)]` on generated methods.** Neither report discusses the
   potential code-bloat impact of forced inlining for structs with many fields.

6. **Serde attribute forwarding risk.** If the original struct uses
   `#[serde(rename)]` or similar attributes, the generated memento does not
   receive them, creating a silent deserialization mismatch at runtime.

7. **Missing property-based / fuzz testing.** For a serialization-boundary
   library, round-trip invariant testing would significantly increase confidence.

8. **Dependabot only covers GitHub Actions, not Cargo deps.** A small project
   hygiene observation neither report makes.

### Priority Disagreement

The three reports suggest different priorities:

| Priority | Gemini | Codex | Claude |
| --- | --- | --- | --- |
| 1st | Expand features | Fix docs | Fix docs |
| 2nd | Custom derives | Less magic | Custom derives |
| 3rd | Ergonomics | Onboarding | Less serde coupling |

I align more closely with Codex's prioritization. The crate's first problem is
not missing capability but imprecise promises. Fixing the user contract before
expanding the feature surface is the path to trustworthiness.

### Synthesis

Taken together, the three reports paint a consistent picture: `recallable` is a
disciplined, well-tested `0.1.0` crate with strong engineering fundamentals and
a user-facing contract that needs tightening. The macro implementation is
genuinely good. The risk is in the gap between what the README promises, what
the macros silently require, and what users will actually experience.

Gemini's feature roadmap is the right *eventual* direction. Codex's contract
precision is the right *immediate* fix. My report adds code-level observations
and the partial-update semantics gap that should inform the design of future
features.
