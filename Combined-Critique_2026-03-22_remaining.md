# Combined Project Critique: recallable — Remaining Items

**Date:** 2026-03-22 (filtered 2026-03-23)

**Evaluators:**

- Gemini 3.1 Pro High — Antigravity v1.0
- Codex GPT-5.4
- Claude Opus 4.6 (1M context)

This is a filtered version of the full critique with completed (✅) items removed.
Only open and partially-addressed (⚠️) items remain.

> **Status key:** Items marked with ⚠️ have been partially addressed.
> Unmarked items remain open.

---

## 3. Generic and Lifetime Constraints

### 3.2. Lifetime Rejection Is Broader Than Documented

⚠️ **Partially fixed.** README and doc comments now say "lifetime-parameterized
structs." The compiler error message in `context.rs:99` still says "borrowed
fields" (code change, to be addressed separately).

**Raised by:** Codex, Claude (Gemini notes it as a constraint)

The `validate_generics` function rejects any struct with *any* lifetime
parameter — not just structs with borrowed fields. The error message says
"Recall derives do not support borrowed fields," which is misleading. A
struct like `Foo<'a> { data: PhantomData<&'a ()> }` contains no borrowed
runtime data but is still rejected.

Codex confirmed this with a throwaway crate. All documentation
(`README.md:238`, `README.md:291-302`, the compiler error text) conflates
"borrowed fields" with "lifetime-parameterized structs."

**Recommendation (Codex, Claude):** Change the error message to "Recall
derives do not support lifetime-parameterized structs" and update the README
to match, unless the implementation is broadened to only reject actual borrow
types.

### 3.3. Enum and Union Support

**Raised by:** Gemini

State machines frequently use enums for state representation. The absence of
enum support restricts `recallable` to struct-only architectures. Gemini
recommends investigating enum memento derivation as a future feature.

---

## 4. Hardcoded Memento Derives

⚠️ **Partially fixed.** The implicit `Clone`/`Debug`/`PartialEq` requirements
are now documented in the README ("Requirements & Limitations" section), the
API Reference (`#[derive(Recallable)]` section), and the `derive_recallable`
doc comment. The customization gap (no `memento_derive(...)` attribute) remains
open.

**Raised by:** Gemini, Codex, Claude

The generated memento struct always derives `Clone`, `Debug`, `PartialEq`
(and `Deserialize` when the `serde` feature is enabled). This creates two
problems:

1. **Hidden requirements.** If any field type does not implement all three
   traits, the derive expansion fails with an error pointing at generated
   code, not the user's struct. These requirements are not documented
   anywhere in user-facing materials. Codex calls this "a bad kind of magic"
   — the public API looks minimal (`trait Recallable { type Memento; }`) but
   the derive macro imposes a much larger undocumented contract.

2. **No customization.** Users who need `Eq`, `Hash`, `Default`, or `Ord`
   on the memento cannot get them without manually implementing `Recallable`
   and defining the memento struct themselves, defeating the macro's purpose.

**Recommendation (all three):** Add a `#[recallable(memento_derive(...))]` or
`#[recallable(memento_attr(...))]` attribute to let users control the derived
traits. At minimum, document the implicit `Clone`/`Debug`/`PartialEq`
requirements prominently.

---

## 5. `#[recallable_model]` and Serde Coupling

### 5.3. Asymmetric Serialization Design

**Raised by:** Gemini (positive), Claude (cautious)

The generated memento derives `Deserialize` but not `Serialize`. Gemini views
this as a smart design choice aligned with durable-execution use cases. Claude
agrees it is intentional but flags the structural coupling risk described
above.

---

## 6. Documentation and Onboarding

### 6.4. Missing Examples Directory

⚠️ **Partially fixed.** The commented-out "Running Examples" TODO block in
`CONTRIBUTING.md` has been removed. The `examples/` directory itself is still
missing (requires creating example crates — a separate task).

**Raised by:** Gemini, Codex, Claude

There is no `examples/` directory. `CONTRIBUTING.md:33-39` contains a
commented-out "Running Examples" section with a TODO. For a proc-macro crate,
standalone runnable examples are one of the most effective onboarding tools.

### 6.6. Memento Type Alias Ergonomics

**Raised by:** Gemini

The `<Struct as Recallable>::Memento` syntax is verbose. Providing
documentation hints or examples showing how to create type aliases
(e.g., `type MyMemento = <MyStruct as Recallable>::Memento;`) would reduce
friction.

---

## 7. Testing

### 7.3. Missing Runtime Edge-Case Tests

**Raised by:** Claude only

No tests cover runtime edge cases like deserialization from malformed data or
field reordering between serialization and deserialization. These are the
scenarios most likely to bite users of a persistence-focused library.

### 7.4. Minimal Doc Tests

⚠️ **Partially fixed.** A doc example has been added to the `Recall` trait
(3 doc tests now exist: `Recallable`, `Recall`, `TryRecall`). The
`recallable_model` macro and field-level attributes still have no doc tests.

**Raised by:** Claude only

Only two doc tests exist (on `Recallable` and `TryRecall`). The `Recall`
trait, `recallable_model` macro, and field-level attributes have no doc tests.

### 7.5. No Property-Based or Fuzz Testing

**Raised by:** Claude only

For a crate operating at serialization boundaries, property-based tests
(e.g., `proptest` or `quickcheck`) verifying round-trip invariants would
add significant confidence.

---

## 8. Project Hygiene

### 8.1. Editor Configuration Committed

**Raised by:** Codex, Claude

`.vscode/settings.json` is tracked in git (containing only file associations
for license files). The `.gitignore` excludes `.idea/` but not `.vscode/`,
which is inconsistent.

**Recommendation:** Either commit both editor configs intentionally or ignore
both.

### 8.2. Dependabot Scope

**Raised by:** Claude only

The `dependabot.yaml` covers `github-actions` updates but not Cargo
dependencies. For a crate with pinned `proc-macro2`, `syn`, `quote`, and
`proc-macro-crate` versions, automated dependency updates would be valuable.

---

## 9. Code-Level Observations

### 9.1. `HashMap` Non-Deterministic Iteration

**Raised by:** Claude only

`MacroContext` uses `HashMap<&Ident, TypeUsage>` for `preserved_types`.
`HashMap` iteration order is non-deterministic. Currently safe because
`build_memento_struct_type` iterates `generics.type_params()` in declaration
order and only checks membership, but fragile if the iteration pattern ever
changes.

### 9.2. `SimpleTypeCollector` Over-Collects

**Raised by:** Claude only

The `collect_used_simple_types` visitor walks the entire type tree and
collects the first segment of every `TypePath`. For a type like
`std::collections::HashMap<K, V>`,
it collects `std`, `K`, and `V`. The `std` entry is harmless but the
over-collection is semantically sloppy.

### 9.3. `#[inline(always)]` on Generated Methods

**Raised by:** Claude only

Both the generated `recall` method and the blanket `TryRecall::try_recall`
are marked `#[inline(always)]`. For structs with many fields, forced inlining
could cause code bloat. `#[inline]` (without `always`) would let the compiler
decide.

---

## 10. Prioritized Recommendations

### High Priority

1. ⚠️ Fix lifetime error message: "lifetime-parameterized structs"
   not "borrowed fields." (README and doc comments fixed; compiler
   error message in `context.rs:99` still says "borrowed fields")
   *— Codex, Claude*

### Medium Priority

1. Add `#[recallable(memento_derive(...))]` for user-controlled
   memento derives.
   *— Gemini, Codex, Claude*
2. ⚠️ Create `examples/` directory with runnable examples.
   (CONTRIBUTING.md TODO block removed; directory not yet created)
   *— Gemini, Codex, Claude*
3. Stop tracking `.vscode/settings.json` or add `.vscode/` to
   `.gitignore`.
   *— Codex, Claude*
4. Enhance generic container support (`Option<T>`, `Vec<T>`).
   *— Gemini, Claude*
5. Investigate enum support.
   *— Gemini*

### Low Priority

1. Consider `#[inline]` instead of `#[inline(always)]` for
   generated methods.
   *— Claude*
2. Add property-based round-trip tests for serialization
   invariants.
   *— Claude*
3. ⚠️ Expand doc tests to cover `recallable_model` and
   field attributes. (`Recall` doc test added; `recallable_model`
   and field attributes still missing)
   *— Claude*
4. Consider supporting `Option<T>` for partial-update semantics.
   *— Claude*
5. Add `cargo` ecosystem to dependabot configuration.
   *— Claude*
6. Document memento type-alias pattern to reduce verbosity of
   `<T as Recallable>::Memento`.
   *— Gemini*
7. Extract CI coverage logic from inline shell into a
   maintainable script.
   *— Claude*

---

## 11. Summary

### What All Three Agree On

1. The project is well-engineered internally — clean code, modular macro
   structure, strong CI, broad test coverage, no bugs found.
2. ⚠️ Hardcoded memento derives create hidden requirements and prevent
   customization. (Requirements now documented; customization still missing.)
3. ⚠️ Lifetime error messaging conflates "borrowed fields" with
   "lifetime-parameterized structs." (Documentation fixed; compiler error
   message still says "borrowed fields.")
4. ⚠️ There is no `examples/` directory. (CONTRIBUTING.md TODO removed;
   directory not yet created.)
5. The crate is promising but not yet ready for broad public adoption.

### The Core Tension

Gemini argues the crate's primary bottleneck is **missing capability** —
broader generic support, enum support, customizable derives. Codex and Claude
argue the primary bottleneck is **imprecise promises** — the README, error
messages, and implicit requirements do not accurately represent what the
macros can and cannot do.

These are not contradictory. The recommended path is:

1. **First:** Tighten the user contract — fix documentation, error messages,
   and implicit requirements so users can trust what the crate says it does.
2. **Then:** Expand capability — broader generics, enums, customizable
   derives — on top of a solid, honest foundation.
