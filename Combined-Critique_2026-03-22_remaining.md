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

### 3.2. Enum and Union Support

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

**Naming note:** `memento_derive(...)` and `memento_attr(...)` are proposal
spellings from the critique reports themselves, not names borrowed from one
canonical proc-macro crate. Gemini suggested both forms; Claude explicitly
proposed `memento_derive(...)`. The closest widely used precedents are
`derive_builder` (`#[builder(derive(...))]` plus `#[builder_struct_attr(...)]`
and related `*_attr` hooks) and `typed-builder`
(`#[builder(builder_type(attributes(...)))]`). More broadly, the outer
`#[recallable(...)]` shape follows the same Rust derive-helper pattern used by
crates such as Serde and Strum.

---

## 5. Documentation and Onboarding

### 5.1. Missing Examples Directory

⚠️ **Partially fixed.** The commented-out "Running Examples" TODO block in
`CONTRIBUTING.md` has been removed. The `examples/` directory itself is still
missing (requires creating example crates — a separate task).

**Raised by:** Gemini, Codex, Claude

There is no `examples/` directory. `CONTRIBUTING.md:33-39` contains a
commented-out "Running Examples" section with a TODO. For a proc-macro crate,
standalone runnable examples are one of the most effective onboarding tools.

---

## 6. Testing

### 6.1. Missing Runtime Edge-Case Tests

**Raised by:** Claude only

No tests cover runtime edge cases like deserialization from malformed data or
field reordering between serialization and deserialization. These are the
scenarios most likely to bite users of a persistence-focused library.

### 6.2. Dedicated Field-Attribute Doc Tests Still Missing

⚠️ **Partially fixed.** Doc tests now exist for `Recallable`, `Recall`,
`TryRecall`, `recallable_model`, and the `Recallable`/`Recall` derive macro
re-exports. Dedicated doc tests for the field-level `#[recallable]` and
`#[recallable(skip)]` attributes still do not exist.

**Raised by:** Claude only

Doc coverage is materially better than it was originally, but the field-level
attributes still lack focused doctest coverage. Their behavior is only exercised
indirectly inside broader examples.

### 6.3. No Property-Based or Fuzz Testing

**Raised by:** Claude only

For a crate operating at serialization boundaries, property-based tests
(e.g., `proptest` or `quickcheck`) verifying round-trip invariants would
add significant confidence.

---

## 7. Code-Level Observations

### 7.1. `HashMap` Non-Deterministic Iteration

**Raised by:** Claude only

`MacroContext` uses `HashMap<&Ident, TypeUsage>` for `preserved_types`.
`HashMap` iteration order is non-deterministic. Currently safe because
`build_memento_struct_type` iterates `generics.type_params()` in declaration
order and only checks membership, but fragile if the iteration pattern ever
changes.

### 7.2. `SimpleTypeCollector` Over-Collects

**Raised by:** Claude only

The `collect_used_simple_types` visitor walks the entire type tree and
collects the first segment of every `TypePath`. For a type like
`std::collections::HashMap<K, V>`,
it collects `std`, `K`, and `V`. The `std` entry is harmless but the
over-collection is semantically sloppy.

---

## 8. Prioritized Recommendations

### Medium Priority

1. Add `#[recallable(memento_derive(...))]` for user-controlled
   memento derives.
   *— Gemini, Codex, Claude*
2. ⚠️ Create `examples/` directory with runnable examples.
   (CONTRIBUTING.md TODO block removed; directory not yet created)
   *— Gemini, Codex, Claude*
3. Enhance generic container support (`Option<T>`, `Vec<T>`).
   *— Gemini, Claude*
4. Investigate enum support.
   *— Gemini*

### Low Priority

1. Add property-based round-trip tests for serialization
   invariants.
   *— Claude*
2. ⚠️ Add dedicated doc tests for field-level attributes.
   (`recallable_model` and trait/macro docs are now covered; field-level
   `#[recallable]` and `#[recallable(skip)]` doctests are still missing)
   *— Claude*
3. Consider supporting `Option<T>` for partial-update semantics.
   *— Claude*
4. Document memento type-alias pattern to reduce verbosity of
   `<T as Recallable>::Memento`.
   *— Gemini*
5. Extract CI coverage logic from inline shell into a
   maintainable script.
   *— Claude*

---

## 9. Summary

### What All Three Agree On

1. The project is well-engineered internally — clean code, modular macro
   structure, strong CI, broad test coverage, no bugs found.
2. ⚠️ Hardcoded memento derives create hidden requirements and prevent
   customization. (Requirements now documented; customization still missing.)
3. ⚠️ There is no `examples/` directory. (CONTRIBUTING.md TODO removed;
   directory not yet created.)
4. The crate is promising but not yet ready for broad public adoption.

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
