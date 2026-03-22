# Documentation Consistency Fixes

**Date:** 2026-03-22
**Source:** Combined-Critique_2026-03-22.md (Gemini 3.1 Pro High, Codex GPT-5.4, Claude Opus 4.6)
**Scope:** Documentation-only changes. No code, no new features, no test changes.

---

## Problem

The critique identified 10 documentation issues where user-facing text does not accurately
describe what the code does. The issues fall into three categories:

1. **Inaccurate claims** — README promises things the macros cannot do
2. **Missing documentation** — implicit requirements and behaviors are undocumented
3. **Structural problems** — information is in the wrong place for effective onboarding

## Files Modified

| File                          | Type of change                                                       |
|-------------------------------|----------------------------------------------------------------------|
| `README.md`                   | Full restructure and content fixes                                   |
| `CHANGELOG.md`                | Expand 0.1.0 entry with feature detail                               |
| `CONTRIBUTING.md`             | Remove commented-out TODO block                                      |
| `recallable/src/lib.rs`       | Add doc example to `Recall` trait                                    |
| `recallable-macro/src/lib.rs` | Add notes to `recallable_model` and `derive_recallable` doc comments |

## Files NOT Modified

These critique items require code changes and are out of scope:

- Error message in `context.rs:99` ("borrowed fields" → "lifetime-parameterized structs")
- `#[inline(always)]` → `#[inline]` in generated code
- `.gitignore` / `.vscode/settings.json` cleanup
- `dependabot.yaml` cargo ecosystem addition
- Hardcoded memento derives (adding `memento_derive(...)` attribute)
- `examples/` directory creation

---

## Change 1: README.md — Full Restructure

### New section order

```text
1. Title + badges + one-paragraph description
2. Why Recallable?
3. Table of Contents
4. Features
5. Installation
6. Requirements & Limitations          ← NEW, moved before examples
7. Usage
   - Basic Example
   - Using #[recallable_model]
   - Skipping Fields
   - Nested Recallable Structs
   - Fallible Recalling
8. How It Works
9. API Reference
10. Contributing / License / Related / Changelog
```

### Content fixes within the restructure

#### Fix 1.1 — Features: generic support claim

**Before (line 60):**
> Full support for generic types with automatic trait bound inference

**After:**
> Support for simple generic type parameters (e.g. `T`) with automatic trait bound inference

Rationale: `Option<T>`, `Vec<T>`, associated types, and lifetimes are all rejected by
`extract_recallable_type_name` in `context.rs:201-227`. The word "full" is false.

#### Fix 1.2 — Features: garbled sentence

**Before (lines 52-53):**
> `#[recallable_model]` Attribute Macro: More or creating a derive attribute, inserting
> `Recallable` and `Recall`, and (with default Cargo feature `serde`) `serde::Serialize`

**After:**
> `#[recallable_model]` Attribute Macro: Injects `#[derive(Recallable, Recall)]` and,
> with the default `serde` feature, `#[derive(serde::Serialize)]` plus `#[serde(skip)]`
> on fields marked `#[recallable(skip)]`

#### Fix 1.3 — New "Requirements & Limitations" section (before examples)

This section consolidates constraints that are currently scattered or missing. Content:

1. **Structs only** — enums and unions are not supported.
2. **No lifetime-parameterized structs** — any struct with a lifetime parameter (e.g.
   `Foo<'a>`) is rejected, even if no fields borrow data. (Current README says "borrowed
   fields" which is inaccurate — `validate_generics` rejects *any* lifetime parameter.)
3. **Simple generic types only** — `#[recallable]` fields accept bare type parameters
   (`T`) and concrete multi-segment paths (`mod::Type`). Parameterized types like
   `Option<T>`, `Vec<T>`, and associated types like `<T as Trait>::Assoc` are rejected.
4. **Implicit trait requirements on field types** — the generated memento struct derives
   `Clone`, `Debug`, and `PartialEq` (and `Deserialize` when the `serde` feature is
   enabled). All non-skipped field types must implement these traits, or compilation will
   fail with an error pointing at generated code. *(Currently undocumented anywhere.)*
5. **`#[recallable_model]` attribute ordering** — must appear *before* any attributes it
   needs to inspect (e.g., before `#[derive(Serialize)]`). Attribute macros only see
   attributes that follow them in source order. *(Currently documented only in CLAUDE.md.)*
6. **Serde behavior** — with the default `serde` feature:
   - `#[recallable_model]` injects `#[derive(serde::Serialize)]` and adds `#[serde(skip)]`
     to `#[recallable(skip)]` fields. Adding a manual `#[derive(Serialize)]` is a compile
     error.
   - `#[derive(Recallable)]` makes the memento derive `Deserialize` but not `Serialize`
     (by design — mementos are deserialized from stored state, not serialized directly).
   - Serde attributes like `#[serde(rename = "...")]` on the original struct are NOT
     forwarded to the memento struct.

#### Fix 1.4 — Installation: mention example dependencies

**Before:**
> Add this to your `Cargo.toml`: `recallable = "0.1.0"`
> Check this project's Cargo feature flags...

**After:** Keep the above, then add a note:

> The examples in this README also use `serde`, `postcard`, and `heapless`. Add them
> as dependencies if you want to run the examples:
>
> ```toml
> [dependencies]
> serde = { version = "1", features = ["derive"] }
> postcard = "1"
> heapless = "0.8"
> ```

#### Fix 1.5 — Limitations section removed from old location

The standalone "### Limitations" section (old lines 235-241) is deleted. Its content is
now covered by the "Requirements & Limitations" section above, with more accurate wording.

#### Fix 1.6 — API Reference: lifetime wording

**Before (lines 291, 301):**
> Does not support lifetime parameters (borrowed fields)

**After:**
> Does not support lifetime-parameterized structs

Applied to both `#[derive(Recallable)]` and `#[derive(Recall)]` requirement lists.

#### Fix 1.7 — API Reference: implicit derives documented

Add to the `#[derive(Recallable)]` section:

> The generated memento struct derives `Clone`, `Debug`, and `PartialEq`. With the `serde`
> feature enabled, it also derives `Deserialize`. All non-skipped field types must implement
> these traits.

#### Fix 1.8 — API Reference: `#[recallable_model]` ordering note

Add to the `#[recallable_model]` section:

> **Attribute ordering:** `#[recallable_model]` must appear before any attributes it needs
> to inspect. An attribute macro's input only contains attributes that follow it in source
> order.

#### Fix 1.9 — Usage: `#[recallable_model]` subsection ordering note

Add a brief note after the example code block in "Using `#[recallable_model]`":

> **Note:** `#[recallable_model]` must appear before other derive/attribute macros it needs
> to interact with. See [Requirements & Limitations](#requirements--limitations) for details.

---

## Change 2: CHANGELOG.md — Expand 0.1.0 Entry

**Before:**

```markdown
- Traits: `Recallable`, `Recall` and `TryRecall`
- Procedural macros for defining Memento pattern types and their state restoration behaviors
```

**After:**

```markdown
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
```

---

## Change 3: CONTRIBUTING.md — Remove TODO Block

Delete lines 33-39 (the commented-out "Running Examples" section):

```html
<!-- ### Running Examples

TODO: Will add soon

```bash
cargo run --example example_name
``` -->
```

Nothing replaces it until an `examples/` directory is created (out of scope for this spec).

---

## Change 4: `recallable/src/lib.rs` — Doc Example for `Recall` Trait

The `Recallable` trait has a doc example (lines 22-98). The `TryRecall` trait has a doc
example (lines 117-175). The `Recall` trait (lines 106-110) has only a one-line doc comment
and no example.

Add a short doc example showing basic usage:

```rust
/// A type that can change state by absorbing one companion memento value.
///
/// # Example
///
/// ```rust
/// use recallable::{Recall, Recallable};
///
/// #[derive(Clone, Debug, PartialEq, Recallable, Recall)]
/// struct Counter {
///     count: u32,
///     label: String,
/// }
///
/// let mut counter = Counter { count: 0, label: "hits".into() };
/// let memento = <Counter as Recallable>::Memento { count: 42, label: "visits".into() };
/// counter.recall(memento);
/// assert_eq!(counter.count, 42);
/// assert_eq!(counter.label, "visits");
/// ```
```

Note: this example does NOT use serde, so it works with `--no-default-features`.

---

## Change 5: `recallable-macro/src/lib.rs` — Doc Comment Additions

### 5a: `recallable_model` — attribute ordering note

Add after the existing doc comment (before `pub fn recallable_model`):

```rust
/// **Attribute ordering:** This macro must appear *before* any attributes it needs
/// to inspect. An attribute macro only receives attributes that follow it in source
/// order. For example, `#[derive(Serialize)]` placed above `#[recallable_model]` is
/// invisible to the macro and will cause a duplicate-derive error.
```

### 5b: `derive_recallable` — implicit derive note

Add after the existing doc comment (before `pub fn derive_recallable`):

```rust
/// The generated memento struct always derives `Clone`, `Debug`, and `PartialEq`.
/// When the `serde` feature is enabled, it also derives `serde::Deserialize`.
/// All non-skipped field types must implement these traits.
```

---

## Verification

After all changes:

```bash
cargo test --package recallable                        # default features (serde)
cargo test --package recallable --no-default-features   # without serde
cargo test --package recallable --features impl_from    # impl_from feature
cargo clippy --workspace --all-targets --all-features   # lint
cargo doc --workspace --no-deps                         # doc generation + doc tests
```

All existing tests must pass unchanged. The new `Recall` doc example must compile and pass
as a doc test.
