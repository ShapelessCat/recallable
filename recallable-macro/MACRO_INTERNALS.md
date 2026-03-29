# `recallable-macro` Internals

Developer guide to the procedural macro crate that powers `recallable`.
This document explains the architecture, data flow, and design decisions
so you can navigate and extend the codebase confidently.

---

## Table of Contents

- [High-Level Architecture](#high-level-architecture)
- [Entry Points (`lib.rs`)](#entry-points)
- [Attribute Macro (`model_macro.rs`)](#attribute-macro)
- [The `context` Facade (`context.rs`)](#the-context-facade)
- [Semantic Analysis — the IR Layer](#semantic-analysis--the-ir-layer)
  - [Shared Infrastructure (`internal/shared/`)](#shared-infrastructure)
  - [Struct IR (`internal/structs/`)](#struct-ir)
  - [Enum IR (`internal/enums/`)](#enum-ir)
- [Code Generation](#code-generation)
  - [Memento Type (`memento/`)](#memento-type)
  - [Recallable Impl (`recallable_impl/`)](#recallable-impl)
  - [Recall Impl (`recall_impl/`)](#recall-impl)
  - [From Impl (`from_impl/`)](#from-impl)
- [Generic Retention System](#generic-retention-system)
- [Trait Bound Inference](#trait-bound-inference)
- [Feature Flags](#feature-flags)
- [Design Principles](#design-principles)

---

## High-Level Architecture

The crate follows a three-phase pipeline:

```text
DeriveInput ──► Semantic Analysis (IR) ──► Code Generation (TokenStream)
                     │                            │
              StructIr / EnumIr            memento struct/enum
                                           Recallable impl
                                           Recall impl
                                           From impl (optional)
```

1. **Parse** — `syn` parses the token stream into a `DeriveInput`.
2. **Analyze** — The IR layer (`StructIr` or `EnumIr`) validates the input and
   builds a rich intermediate representation: field strategies, generic retention
   plans, where-clause filtering, and marker param detection.
3. **Generate** — Codegen functions consume `&StructIr`/`&EnumIr` + `&CodegenEnv`
   and emit `TokenStream2` fragments that `lib.rs` assembles into the final output.

The IR is built once and shared across all codegen passes. This avoids redundant
parsing and keeps each codegen module focused on token assembly.

---

## Entry Points

**File:** `src/lib.rs`

Three proc-macro entry points, each a thin shell:

| Macro                   | Function              | What it does                                                                         |
|-------------------------|-----------------------|--------------------------------------------------------------------------------------|
| `#[recallable_model]`   | `recallable_model()`  | Attribute macro — delegates to `model_macro::expand`                                 |
| `#[derive(Recallable)]` | `derive_recallable()` | Analyzes input → generates memento type + `Recallable` impl (+ optional `From` impl) |
| `#[derive(Recall)]`     | `derive_recall()`     | Analyzes input → generates `Recall` impl                                             |

All generated code is wrapped in `const _: () = { ... };` blocks with
`#[automatically_derived]` to avoid polluting the item namespace and to signal
to lints that the code is machine-generated.

---

## Attribute Macro

**File:** `src/model_macro.rs`

`#[recallable_model]` is a convenience attribute that:

1. Validates it receives no arguments.
2. Parses the item as a struct or enum via `ModelItem`.
3. Runs `context::analyze_model_input()` to reject unsupported enum shapes.
4. Checks for duplicate `Serialize` derives (serde feature only).
5. Injects `#[derive(Recallable, Recall)]` (and `serde::Serialize` when serde is enabled).
6. Adds `#[serde(skip)]` to fields marked `#[recallable(skip)]`.

`ModelItem` is a local enum over `ItemStruct` / `ItemEnum` that provides a
uniform interface for attribute manipulation (`add_derives`, `add_serde_skip_attrs`,
`parse` -> `DeriveInput`).

The duplicate-Serialize check scans `#[derive(...)]` attributes for paths ending
in `Serialize`, matching `serde::Serialize`, `serde_derive::Serialize`, and bare
`Serialize`. This catches the most common user mistake since `#[recallable_model]`
auto-adds `Serialize` when serde is enabled.

> **Attribute ordering:** `#[recallable_model]` must appear *before* any attributes
> it needs to inspect. An attribute macro's `item` token stream only contains
> attributes that follow it in source order.

---

## The `context` Facade

**File:** `src/context.rs`

A thin coordination layer that:

- Re-exports IR types and codegen functions for `lib.rs` to consume.
- Defines feature-flag constants: `SERDE_ENABLED` and `IMPL_FROM_ENABLED`.
- Provides three analysis entry points:
  - `analyze_item()` — builds `ItemIr` (struct or enum).
  - `analyze_recall_input()` — same, but also validates enum recall-derive eligibility.
  - `analyze_model_input()` — validates enum model-attribute eligibility.
- Delegates codegen to submodules: `memento`, `recallable_impl`, `recall_impl`, `from_impl`.

Each codegen submodule has a top-level dispatcher that matches on `ItemIr::Struct` /
`ItemIr::Enum` and calls the appropriate struct- or enum-specific generator.

---

## Semantic Analysis — the IR Layer

### Shared Infrastructure

**Directory:** `src/context/internal/shared/`

This is the foundation that both `StructIr` and `EnumIr` build on.

#### `item.rs` — `ItemIr` enum

```rust
enum ItemIr<'a> {
    Struct(StructIr<'a>),
    Enum(EnumIr<'a>),
}
```

The top-level dispatch type. `ItemIr::analyze()` routes to the appropriate IR
constructor based on `syn::Data`. Also handles item-level attribute parsing
(`skip_memento_default_derives`).

#### `fields.rs` — Field analysis

Core types:

- **`FieldStrategy`** — `Skip | StoreAsSelf | StoreAsMemento`. Determines how each
  field participates in the memento.
- **`FieldIr`** — Per-field IR: source reference, memento index (position in the
  generated memento after skipped fields are removed), member accessor, type, strategy.
- **`FieldMember`** — `Named(&Ident) | Unnamed(Index)`. Implements `ToTokens` for
  use in both pattern and expression positions.

Field classification logic:

1. Parse `#[recallable]` and `#[recallable(skip)]` attributes → `FieldBehavior`.
2. Conflicting `#[recallable]` + `#[recallable(skip)]` on the same field → compile error.
3. `PhantomData` fields → auto-skipped (regardless of attributes).
4. `#[recallable]` on a bare type param `T` → `StoreAsMemento` with the param
   flagged as `RetainedAsRecallable`.
5. `#[recallable]` on a complex path type like `Option<T>` → `StoreAsMemento`
   with whole-type bounds (no bare-param shorthand).
6. Everything else → `StoreAsSelf`.

`collect_field_irs()` processes all fields and returns both the field IR vec and
a `GenericUsage` summary (which generic params are retained, which are recallable).

#### `generics.rs` — Generic retention

The most complex piece of the analysis. See [Generic Retention System](#generic-retention-system)
for the full algorithm.

Key types:

- **`GenericParamRetention`** — `Dropped | Retained | RetainedAsRecallable`.
- **`GenericParamPlan`** — Pairs a `&GenericParam` with its retention decision.
  Provides `decl_param()`, `type_arg()`, `recallable_ident()`.
- **`GenericParamLookup`** — Index from ident → param position, split by kind
  (type, const, lifetime). Used during field analysis and dependency collection.
- **`GenericUsage`** — Accumulated from field analysis: which param indices are
  retained, which are recallable type params.
- **`GenericDependencyCollector`** — A `syn::Visit` walker that collects which
  generic params a given type or where-predicate depends on.

#### `lifetime.rs` — Borrowed-field rejection

- `collect_item_lifetimes()` — extracts lifetime param idents from generics.
- `validate_no_borrowed_fields()` — rejects non-`PhantomData`, non-skipped fields
  that reference struct lifetimes. Uses `LifetimeUsageChecker` (a `syn::Visit` walker).
- `is_phantom_data()` — heuristic path match: any path ending in `PhantomData`.

#### `bounds.rs` — Trait bound assembly

- **`MementoTraitSpec`** — Encapsulates the derive/bound configuration for the
  memento type. Controlled by two flags: `serde_enabled` and `derive_off`
  (from `#[recallable(skip_memento_default_derives)]`).
  - `derive_attr()` → the `#[derive(...)]` attribute for the memento.
  - `common_bound_tokens()` → `Clone + Debug + PartialEq` (or empty if derive_off).
  - `serde_nested_bound()` → `DeserializeOwned` (or None if serde disabled).
- `collect_shared_memento_bounds()` — Builds where-predicates for memento type
  definitions: `T::Memento: Clone + Debug + PartialEq` and whole-type equivalents.
- `collect_recall_like_bounds()` — Builds where-predicates for trait impls
  (`Recallable`, `Recall`): direct bounds on recallable params + shared memento bounds.

#### `codegen.rs` — Shared codegen helpers

- **`CodegenItemIr`** trait — The polymorphism layer. Both `StructIr` and `EnumIr`
  implement this trait, providing uniform access to generics, fields, memento name,
  marker params, etc. This lets bound-collection and field-token-building code work
  generically across item kinds.
- `build_memento_field_ty()` — Produces the memento field type: `T::Memento` for
  bare type params, `<FieldType as Recallable>::Memento` for complex paths,
  or the original type for `StoreAsSelf`.
- `build_memento_field_tokens()` — Wraps the type with the field name for named
  fields, or emits just the type for tuple fields.
- `build_from_value_expr()` — Wraps an expression in `From::from()` for
  `StoreAsMemento` fields, passes through for `StoreAsSelf`.

#### `env.rs` — `CodegenEnv`

```rust
struct CodegenEnv {
    recallable_trait: TokenStream2,  // e.g. ::recallable::Recallable
    recall_trait: TokenStream2,      // e.g. ::recallable::Recall
}
```

Resolved once per macro invocation via `crate_path()`. Holds the fully-qualified
trait paths used throughout codegen.

#### `util.rs` — Crate path resolution

`crate_path()` uses `proc-macro-crate` to resolve the `recallable` crate name,
handling re-exports and the `Itself` case (when the macro is invoked from within
the `recallable` crate itself).

### Struct IR

**Directory:** `src/context/internal/structs/`

#### `ir.rs` — `StructIr`

The struct-specific IR. Built by `StructIr::analyze()`:

1. Collect item lifetimes, validate no borrowed fields.
2. Determine `StructShape` (`Named | Unnamed | Unit`).
3. Build `GenericParamLookup`, collect field IRs + generic usage.
4. Run `plan_memento_generics()` → generic param plans + filtered where-clause.
5. Detect marker param indices (retained params not referenced by any kept field).
6. Parse `skip_memento_default_derives`.

Key accessors: `struct_type()`, `memento_name()`, `visibility()`, `shape()`,
`impl_generics()`, `memento_fields()` (iterator over non-skipped fields),
`has_synthetic_marker()`.

Implements `CodegenItemIr` for polymorphic codegen.

#### `bounds.rs` — Struct-specific bound wrappers

Thin wrappers around the shared `collect_shared_memento_bounds` and
`collect_recall_like_bounds`, automatically passing the struct's `MementoTraitSpec`.

### Enum IR

**Directory:** `src/context/internal/enums/`

#### `ir.rs` — `EnumIr` and `VariantIr`

The enum-specific IR. `EnumIr::analyze()` follows the same pattern as `StructIr`
but processes variants:

1. Each variant's fields are analyzed independently via `collect_field_irs()`.
2. Variants are classified by `VariantShape` (`Named | Unnamed | Unit`).
3. Generic usage is merged across all variants.

**`VariantIr`** holds per-variant data: name, shape, field IRs. Provides
`kept_fields()` (non-skipped) and `kept_bindings()` (binding idents for pattern matching).

**Enum restrictions:**

- `ensure_recall_derive_allowed()` — Rejects enums with `#[recallable]` fields or
  non-`PhantomData` skipped fields. These "complex" enums must implement `Recall`
  or `TryRecall` manually.
- `ensure_model_derive_allowed()` — Same restriction for `#[recallable_model]`.
- `supports_derived_recall()` — Returns whether the restore helper should be generated.

`build_binding_ident()` creates binding names for pattern matching: named fields
keep their ident, unnamed fields get `__recallable_field_{index}`.

Implements `CodegenItemIr` with `all_fields()` flat-mapping across all variants.

#### `bounds.rs` — Enum-specific bound wrappers

Same pattern as struct bounds — thin wrappers passing the enum's `MementoTraitSpec`.

---

## Code Generation

Each codegen module has a top-level dispatcher in its `mod.rs` that matches on
`ItemIr` and delegates to struct- or enum-specific generators.

### Memento Type

**Directory:** `src/context/memento/`

Generates the companion memento type that mirrors the source item.

**Structs** (`structs.rs`):

- Emits `#[derive(Clone, Debug, PartialEq)]` (+ `Deserialize` with serde).
- Mirrors the struct shape (named/unnamed/unit).
- Field types: original type for `StoreAsSelf`, `T::Memento` or
  `<Type as Recallable>::Memento` for `StoreAsMemento`.
- All fields are private (no visibility modifiers).
- Appends a synthetic `_recallable_marker: PhantomData<(...)>` field when
  retained generic params aren't referenced by any kept field.
- Where-clause includes `Recallable` bounds + memento trait bounds.

**Enums** (`enums.rs`):

- Same derive/visibility/bound strategy as structs.
- Each variant mirrors the source variant shape, with skipped fields removed.
- Variants that lose all fields become unit variants.
- Synthetic marker uses a hidden `__RecallableMarker(PhantomData<(...)>)` variant
  with `#[serde(skip)]` when serde is enabled.

### Recallable Impl

**Directory:** `src/context/recallable_impl/`

Generates `impl Recallable for Type { type Memento = TypeMemento; }`.

**Structs** (`structs.rs`): Straightforward — emits the impl with appropriate
generics and where-clause.

**Enums** (`enums.rs`): Additionally generates a private
`__recallable_restore_from_memento()` helper method on the enum type (when the
enum supports derived recall). This helper contains the `match` expression that
reconstructs enum values from memento variants. It's emitted alongside the
`Recallable` impl so that `Recall` can call it.

The restore helper:

- Matches each memento variant and reconstructs the corresponding source variant.
- `StoreAsSelf` fields pass through directly.
- Skipped `PhantomData` fields are reconstructed as `PhantomData`.
- The `__RecallableMarker` variant arm is `unreachable!()`.

### Recall Impl

**Directory:** `src/context/recall_impl/`

Generates `impl Recall for Type { fn recall(&mut self, memento) { ... } }`.

**Structs** (`structs.rs`):

- For each non-skipped field:
  - `StoreAsSelf` → `self.field = memento.field;`
  - `StoreAsMemento` → `Recall::recall(&mut self.field, memento.field);`
- Memento parameter is named `_memento` when there are no fields to recall.
- The `recall` method is `#[inline]`.

**Enums** (`enums.rs`):

- Delegates to `*self = Self::__recallable_restore_from_memento(memento);`
- The actual reconstruction logic lives in the restore helper generated by
  `recallable_impl`.

### From Impl

**Directory:** `src/context/from_impl/`

Generates `impl From<Type> for TypeMemento` (behind the `impl_from` feature).

**Structs** (`structs.rs`):

- Named: `Self { field: value.field, ... }`
- Unnamed: `Self(value.0, ...)`
- Unit: `Self`
- `StoreAsMemento` fields wrapped in `From::from()`.
- Synthetic marker initialized as `PhantomData`.

**Enums** (`enums.rs`):

- Match on source enum, construct corresponding memento variant.
- Binding idents used for destructuring and reconstruction.
- Skipped fields matched as `_` in patterns.

---

## Generic Retention System

The generic retention algorithm determines which generic parameters from the
source type appear on the generated memento. This is the most intricate part
of the analysis.

### Algorithm

1. **Field analysis** (`collect_field_irs`) records which generic params each
   non-skipped field depends on → `GenericUsage.retained`.

2. **Fixed-point dependency closure** (`plan_memento_generics`):
   - Start with the set of params directly used by kept fields.
   - Scan where-clause predicates: if a predicate depends on any retained param,
     mark all params it references as retained too.
   - Repeat until no new params are added (fixed-point).
   - This ensures transitive dependencies are captured. For example, if `T: Into<U>`
     and `T` is retained, then `U` must also be retained.

3. **Where-clause filtering**: Only predicates that depend on at least one retained
   param are kept on the memento's where-clause. Predicates involving only dropped
   params are removed.

4. **Retention classification**:
   - `Dropped` — param not used by any kept field (directly or transitively).
   - `Retained` — param used by a `StoreAsSelf` field.
   - `RetainedAsRecallable` — type param used by a `StoreAsMemento` field
     (gets `Recallable` bounds).

5. **Marker params** (`collect_marker_param_indices`): Retained params that aren't
   referenced by any kept field's type need a `PhantomData` marker to satisfy
   Rust's "must use all generic params" rule. This happens when a param is only
   referenced through where-clause dependencies.

### Marker field generation

- **Structs**: A `_recallable_marker: PhantomData<(T, U, ...)>` field is appended.
  For named structs it's a named field; for tuple structs it's a positional field.
- **Enums**: A hidden `__RecallableMarker(PhantomData<(T, U, ...)>)` variant is added,
  with `#[doc(hidden)]` and `#[serde(skip)]`.
- Const generic params in the marker use a helper type alias
  `type __RecallableConstMarker<const N: ...> = [(); N];` to embed them in `PhantomData`.

---

## Trait Bound Inference

The macro automatically infers trait bounds for generic parameters. Bounds are
assembled in layers:

### For the memento type definition

```rust
T: Recallable                              // bare recallable params
T::Memento: Clone + Debug + PartialEq      // common trait bounds (unless derive_off)
T::Memento: DeserializeOwned               // serde feature only
ComplexType: Recallable                    // whole-type recallable bounds
<ComplexType as Recallable>::Memento: ...  // whole-type memento bounds
```

### For `Recallable` / `Recall` impls

Same structure, but the direct bound varies:

- `Recallable` impl uses `T: Recallable` as the direct bound.
- `Recall` impl uses `T: Recall` as the direct bound.

### For `From` impl

```rust
T: Recallable
T::Memento: From<T>                       // conversion bound
// + shared memento bounds
// + whole-type From bounds
```

The `collect_recall_like_bounds()` function in `shared/bounds.rs` is the
workhorse — it's parameterized by the "direct bound" trait and assembles
the full predicate list.

---

## Feature Flags

Feature flags are evaluated at compile time of the macro crate itself (not the
downstream user's crate):

```rust
// context.rs
pub(super) const SERDE_ENABLED: bool = cfg!(feature = "serde");
pub(super) const IMPL_FROM_ENABLED: bool = cfg!(feature = "impl_from");
```

These are module-level constants, not part of `CodegenEnv`. They gate:

| Flag        | Effect                                                                        |
|-------------|-------------------------------------------------------------------------------|
| `serde`     | 1. Memento derives `Deserialize`;                                             |
|             | 2. `#[recallable_model]` adds `Serialize`; `#[serde(skip)]` on marker fields; |
|             | 3. `DeserializeOwned` bounds on nested mementos                               |
|-------------|-------------------------------------------------------------------------------|
| `impl_from` | `From<Type>` impl generated for memento types                                 |

---

## Design Principles

1. **Analyze once, generate many.** The IR is the single source of truth. Each
   codegen module is a pure function from `(&IR, &CodegenEnv) -> TokenStream`.

2. **Polymorphism via `CodegenItemIr`.** Shared codegen logic (generics, bounds,
   field tokens) works across structs and enums through a trait, avoiding
   duplication while keeping item-specific code in dedicated modules.

3. **Dependency-closed generics.** The fixed-point loop ensures the memento's
   generic parameter set is self-consistent — no dangling bounds or missing params.

4. **Opaque mementos.** Generated memento fields are always private. Mementos are
   state tokens, not an inspection surface.

5. **Heuristic type matching.** The macro can't resolve types, so it uses path-based
   heuristics (e.g., `is_phantom_data` matches any path ending in `PhantomData`,
   `is_generic_type_param` checks single-segment paths against known type params).

6. **Graceful enum restrictions.** Complex enums (with `#[recallable]` fields or
   non-phantom skipped fields) are rejected with a clear error message directing
   users to manual `Recall`/`TryRecall` implementations. Simple "assignment-only"
   enums get full derive support.

7. **Hygienic output.** All generated code lives in `const _: () = { ... }` blocks,
   uses fully-qualified paths (`::core::...`, `::serde::...`), and is marked
   `#[automatically_derived]`.

---

## Module Map

```text
recallable-macro/src/
├── lib.rs                          # proc-macro entry points
├── model_macro.rs                  # #[recallable_model] expansion
├── context.rs                      # facade: re-exports, feature flags, analysis entry points
└── context/
    ├── memento.rs                  # ItemIr dispatcher
    ├── memento/
    │   ├── structs.rs              # memento struct generation
    │   └── enums.rs                # memento enum generation
    ├── recallable_impl.rs          # ItemIr dispatcher
    ├── recallable_impl/
    │   ├── structs.rs              # Recallable impl for structs
    │   └── enums.rs                # Recallable impl + restore helper for enums
    ├── recall_impl.rs              # ItemIr dispatcher
    ├── recall_impl/
    │   ├── structs.rs              # Recall impl for structs
    │   └── enums.rs                # Recall impl for enums (delegates to restore helper)
    ├── from_impl.rs                # ItemIr dispatcher
    ├── from_impl/
    │   ├── structs.rs              # From impl for structs
    │   └── enums.rs                # From impl for enums
    ├── internal.rs                 # re-exports shared/structs/enums
    └── internal/
        ├── shared.rs               # re-exports all shared types
        ├── shared/
        │   ├── item.rs             # ItemIr enum, item-level attr parsing
        │   ├── fields.rs           # FieldIr, FieldStrategy, field classification
        │   ├── generics.rs         # generic retention algorithm, dependency collector
        │   ├── lifetime.rs         # borrowed-field rejection, PhantomData detection
        │   ├── bounds.rs           # MementoTraitSpec, bound collection
        │   ├── codegen.rs          # CodegenItemIr trait, shared field/type helpers
        │   ├── env.rs              # CodegenEnv (resolved crate paths)
        │   └── util.rs             # crate_path(), is_recallable_attr()
        ├── structs.rs              # re-exports
        ├── structs/
        │   ├── ir.rs               # StructIr, StructShape
        │   └── bounds.rs           # struct-specific bound wrappers
        ├── enums.rs                # re-exports
        └── enums/
            ├── ir.rs               # EnumIr, VariantIr, VariantShape
            └── bounds.rs           # enum-specific bound wrappers
```
