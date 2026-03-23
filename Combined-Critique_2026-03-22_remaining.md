# Combined Project Critique: recallable — Remaining Items

**Date:** 2026-03-22 (revalidated 2026-03-24)

**Evaluators:**

- Gemini 3.1 Pro High — Antigravity v1.0
- Codex GPT-5.4
- Claude Opus 4.6 (1M context)

This is a filtered version of the full critique with completed items removed.
Only open and partially addressed items remain, rechecked against the current codebase.

> **Status key:** Items marked with ⚠️ have been partially addressed.
> Unmarked items remain open.

---

## 1. Generic and Lifetime Constraints

### 1.1. Enum and Union Support

**Raised by:** Gemini

State machines frequently use enums for state representation. The absence of
enum support restricts `recallable` to struct-only architectures. Gemini
recommends investigating enum memento derivation as a future feature.

---

## 2. Hardcoded Memento Derives

⚠️ **Partially fixed.** The implicit `Clone`/`Debug`/`PartialEq` requirements
are now documented in the README ("Requirements & Limitations" and "API Reference"),
the `recallable` crate docs, and the `derive_recallable` doc comment in
`recallable-macro`. The remaining gap is architectural: the derive list is still
hardcoded, and missing trait support still surfaces as errors in generated code.

**Raised by:** Gemini, Codex, Claude

The generated memento struct always derives `Clone`, `Debug`, `PartialEq`
(and `Deserialize` when the `serde` feature is enabled). Two issues remain:

1. **No customization.** Users who need `Eq`, `Hash`, `Default`, or `Ord`
   on the memento still cannot get them without manually implementing
   `Recallable` and defining the memento struct themselves.

2. **Diagnostics are still indirect.** The requirements are documented now,
   but if a field type is missing a required trait the failure still points at
   generated code rather than at an explicit user-authored configuration site.

**Recommendation (all three):** Add a `#[recallable(memento_derive(...))]` or
`#[recallable(memento_attr(...))]` attribute to let users control the derived
traits. At minimum, keep the current documentation prominent and consider
improving diagnostics around missing trait implementations.

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

## 3. Prioritized Recommendations

### High Priority

### Medium Priority

1. Add `#[recallable(memento_derive(...))]` for user-controlled
   memento derives.
   *— Gemini, Codex, Claude*
2. Investigate enum support.
   *— Gemini*

---

## 4. Summary

### What All Three Still Agree On

1. The project is well-engineered internally: modular macro code, broad happy-path
   test coverage, and no concrete correctness bugs found in the current implementation.
2. ⚠️ Hardcoded memento derives still prevent customization. Documentation is much
   better now, but the macro contract remains more rigid than the public trait
   surface suggests.
3. The crate is promising, but a few capability and productization gaps remain
   before broad public adoption.

### The Current Core Tension

The earlier documentation/contract gap has narrowed substantially: README and
API docs now spell out the important current limitations and trait requirements.
What remains is a smaller, clearer split:

- Gemini is still mostly pointing at **missing capability**:
  broader generic support, enum support, customizable derives.
- Codex is still mostly pointing at **productization gaps**:
  indirect diagnostics around hardcoded derives.
- Claude is now mostly pointing at **implementation robustness and maintainability**:
  internal representation fragility and overly broad type collection.

These are complementary. The practical path from here is:

1. **First:** Close the productization gaps that still affect user trust:
   better diagnostics around hardcoded derives.
2. **Then:** Expand capability on top of that clearer baseline:
   enum support, and customizable memento derives.
