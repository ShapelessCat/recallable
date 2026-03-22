# Project Critique: Recallable

**Date:** 2026-03-22 **Evaluator:** Antigravity (Version 1.0)

## Executive Summary

`recallable` is a well-structured implementation of the Memento design pattern
in Rust. It utilizes procedural macros to remove the boilerplate associated with
defining companion state structures (mementos) for partial updates. The project
excels in its comprehensive test coverage, clean code quality, and integration
with `serde`. However, some macro constraints—particularly around complex
generic types and hardcoded derived traits on the generated memento—present
opportunities for further refinement.

---

## 1. Architecture and Design Choices

### Architectural Strengths

- **Clean Trait Hierarchy:** The separation of concerns between `Recallable`
  (declaring the associated Memento type), `Recall` (infallible updates), and
  `TryRecall` (fallible updates) forms a logical and idiomatic API boundary. The
  blanket implementation for `TryRecall` on all `Recall` types is an excellent
  developer experience detail.
- **Invisible Generation:** Automatically generating the companion Memento
  struct as an implementation detail (`<Struct as Recallable>::Memento`) is a
  clever way to prevent namespace pollution.
- **Smart Serde Integration:** The `serde` integration is tight. Automatically
  deriving `Deserialize` for the memento but not `Serialize` (as mementos are
  meant to be received, not sent back out, or sent as a partial update) aligns
  perfectly with typical durable-execution and partial-update use cases.
  Furthermore, intelligently injecting `#[serde(skip)]` on fields with
  `#[recallable(skip)]` via the `recallable_model` macro ensures alignment
  between serialization and state-restoration.

### Constraints & Limitations

- **Complex Generics:** `#[recallable]` fields currently only support simple
  generic type arguments (`T`). Standard data structures like `Vec<T>`,
  `Option<T>`, or `HashMap<K, V>` cannot recursively delegate recalling. In many
  event-driven pipelines, nested collections of recallable items are incredibly
  common.
- **Lack of Enum Support:** State machines often use Enums for state
  representation. While supporting enums involves more complex code generation
  (matching variants), its absence restricts `recallable` strictly to
  struct-based structures.
- **Lifetime Parameters:** The lack of lifetime support limits use cases where
  state might contain borrows (though mementos generally require owned data for
  persistence, zero-copy deserialization architectures often rely heavily on
  borrowed data).

---

## 2. Code Quality and Extensibility

### Macro Implementation

The `recallable-macro` code is modular and well-abstracted:

- Breaking the context generation into `context.rs`, `memento_struct.rs`,
  `from_impl.rs` etc., makes the macro easy to read and maintain.
- The automatic inference of trait bounds for `Recallable` is properly executed,
  avoiding the over-constraining issues typical in many standard library
  derives.
- `proc-macro-crate` is sensibly used to ensure the generated code works
  correctly if the consumer renames the crate in `Cargo.toml`.

### Hardcoded Memento Derives

The macro hardcodes `#[derive(Clone, Debug, PartialEq)]` onto the generated
memento struct (plus `Deserialize` if `serde` is enabled).

- **Critique:** The user cannot customize the derived traits on the Memento.
  What if they need `Eq`, `Hash`, or `Default`? Conversely, what if the inner
  data types don't implement `PartialEq` or `Debug`, causing a compilation
  error? A mechanism to forward or specify attributes (e.g.,
  `#[recallable(memento_derive(Eq, Hash))]`) would make the library more robust.

---

## 3. Ergonomics and Developer Experience

### Ergonomic Strengths

- **MSRV and CI:** Excellent commitment to maintaining an MSRV alongside stable,
  backed by rigorous CI configuration and `trybuild` UI tests. The recent
  proactive bump to Rust 1.88 (edition 2024) and adoption of modern language
  features like `let-chains` showcases a mature, forward-looking maintenance
  strategy.
- **Safety:** The macro restricts generic parsing proactively with clear
  `compile_error!` messages when constraints (like lifetimes or missing structs)
  are violated, leading to better user feedback.
- **No_Std:** Opting into `#![no_std]` is wonderful for embedded environments or
  specialized runtimes.

### Areas for Ergonomics Improvements

- **Verbosity of Associate Types:** Providing a user-friendly alias or
  documentation hint on how to alias `<Struct as Recallable>::Memento` could
  reduce syntax clutter (e.g. `type MyMemento = ...;`).
- **Multi-Attribute Sequencing Issue:** As noted in `CLAUDE.md`,
  `#[recallable_model]` must appear before other attributes. While logical due
  to macro token stream execution, this can trip up users. A dedicated test case
  or prominent README note highlighting this is vital.

---

## 4. Recommendations for Next Steps

1. **Enhance Generic Container Support:** Add macro logic to detect common
   wrapper types (like `Option`, `Vec`) so `#[recallable]` can iterate and
   recall inner items recursively. This would significantly jumpstart the
   library’s applicability.
2. **Customizable Memento Attributes:** Add a `#[recallable(memento_attr(...))]`
   or similar directive to pass traits out to the invisible memento struct,
   moving away from hardcoded derives.
3. **Enum Support Investigation:** Consider implementing Memento derivation for
   Enums. This would involve generating mirroring Enums to reflect structural
   changes across state variant states.

**Summary:** `recallable` is a clean, hyper-focused crate solving a specific
persistence edge case seamlessly. Expanding its macro limits will unlock its
usage for much larger, complex state-management architectures.
