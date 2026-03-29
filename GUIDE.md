# Recallable Guide and Reference

`recallable` is a `no_std`-friendly crate for Memento-pattern state updates and
recovery in Rust.
It is designed for types that already exist at runtime and need to absorb
durable state from a companion value without reconstructing the whole object.

This guide is the long-form user-facing reference for the crate.
It covers the intended workflows, macro behavior, feature flags, supported type
shapes, important caveats, and the public API surface.

## Table of Contents

- [Overview](#overview)
- [When Recallable Fits](#when-recallable-fits)
- [Why Not Just Deserialize?](#why-not-just-deserialize)
- [Core Concepts](#core-concepts)
- [Choosing a Workflow](#choosing-a-workflow)
- [Installation and Features](#installation-and-features)
- [Quickstart](#quickstart)
- [Using `#[recallable_model]`](#using-recallable_model)
- [Using direct derives](#using-direct-derives)
- [Skipped fields and memento visibility](#skipped-fields-and-memento-visibility)
- [Recursive fields and container-defined semantics](#recursive-fields-and-container-defined-semantics)
- [Fallible recall with `TryRecall`](#fallible-recall-with-tryrecall)
- [In-memory snapshots with `impl_from`](#in-memory-snapshots-with-impl_from)
- [Manual trait implementations](#manual-trait-implementations)
- [Supported shapes, generics, and lifetimes](#supported-shapes-generics-and-lifetimes)
- [Serialization guidance](#serialization-guidance)
- [Macro and trait reference](#macro-and-trait-reference)
- [Design guarantees](#design-guarantees)
- [Current limitations](#current-limitations)
- [Examples and project files](#examples-and-project-files)
- [Contributing, license, and changelog](#contributing-license-and-changelog)

## Overview

The crate exposes three traits:

- `Recallable` declares an associated `Memento` type
- `Recall` applies a memento infallibly
- `TryRecall` applies a memento fallibly with validation

It also provides procedural macros for the common case where an ordinary struct
should have a generated companion memento type and generated recall logic.

The key idea is simple:

1. keep a runtime struct alive
2. separate durable fields from runtime-only fields
3. deserialize or construct a memento
4. apply the memento to the live value

That is different from ordinary deserialization, which builds a brand-new value.

## When Recallable Fits

Recallable works well when:

- only part of a runtime struct is durable state
- you want a typed state token instead of handwritten patch structs
- state must be restored into long-lived in-memory objects
- runtime-only fields must survive updates unchanged
- nested fields should apply their own recall semantics recursively
- the durable representation may be persisted or sent over the wire

Common examples include:

- durable execution and workflow engines
- event-sourced systems
- embedded state machines
- services with connection handles or caches
- streaming pipelines with runtime-only helpers

## Why Not Just Deserialize?

Standard `Deserialize` constructs a new value.
That is often the wrong operation when you already have a live runtime object
with fields that should survive updates unchanged.

If your type has fields like:

- caches
- connection handles
- closures or function pointers
- runtime-only counters or derived state

then "deserialize a fresh value" tends to force one of two bad outcomes:

- invent a meaningless default value for runtime-only fields
- push reconstruction logic into places where it does not belong

Recallable solves a different problem:

1. keep the runtime object
2. decode or build a memento
3. apply only the durable state
4. preserve the skipped runtime-only fields

That is the core reason the crate is built around applying mementos rather than
reconstructing values from scratch.

## Core Concepts

### A memento is a companion state type

`Recallable` does not say how a type should export or serialize state.
It only says that a type has a companion `Memento`.

For simple structs, the generated memento usually looks like a copy of the
durable fields.
For container-like types, the memento shape is intentionally application-defined.

### Recall is apply-side behavior

`Recall::recall` means "absorb this memento into the current value".
That can mean replacement, merging, selective nested updates, or any other
domain-specific behavior chosen by the type.

### Runtime-only fields are explicit

Fields marked `#[recallable(skip)]` are omitted from generated mementos and left
untouched during recall.
This is the main mechanism for keeping caches, handles, closures, or other
non-durable runtime state alive.

## Choosing a Workflow

There are two main ways to use the crate.

### Workflow A: persistence and restore

Use this when state crosses process boundaries or is written to disk.

Preferred flow:

1. serialize the source struct
2. store or transmit the encoded state
3. deserialize into `<Type as Recallable>::Memento`
4. apply the memento with `recall` or `try_recall`

This is the default happy path for `#[recallable_model]`, because it keeps the
source struct's serialized shape aligned with the generated memento shape.

### Workflow B: in-memory snapshots

Use this when you want an owned memento value within the same process:

- checkpoint and rollback
- undo stacks
- test fixtures
- state handoff between components

Enable the `impl_from` feature to derive `From<Type>` for the generated
memento type.

## Installation and Features

Base dependency:

```toml
[dependencies]
recallable = "0.2.0"
```

MSRV is Rust 1.88 with edition 2024.

Feature flags:

- `serde` (default): enables macro-generated serde support; generated mementos
  derive `serde::Deserialize`, and `#[recallable_model]` also injects
  source-side serde behavior. This feature remains compatible with `no_std` as
  long as your serde stack is configured for `no_std`.
- `impl_from`: generates `From<Type>` for the generated memento
- `full`: convenience feature for `serde` + `impl_from`
- `default-features = false`: disables recallable's default serde integration.
  It is useful for non-serde setups, but it is not what makes `no_std`
  possible.

Example dependency sets:

```toml
[dependencies]
# Readable std example
recallable = "0.2.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```toml
[dependencies]
# no_std + serde example
recallable = { version = "0.2.0", default-features = false, features = ["serde"] }
serde = { version = "1", default-features = false, features = ["derive"] }
postcard = { version = "1", default-features = false, features = ["heapless"] }
heapless = { version = "0.9.2", default-features = false }
```

## Quickstart

The most ergonomic starting point is `#[recallable_model]` with the default
`serde` feature.

```rust
use recallable::{Recall, Recallable, recallable_model};

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
struct DashboardState {
    volume: u8,
    label: String,
    #[recallable(skip)]
    cache_key: String,
}

fn main() {
    let mut dashboard = DashboardState {
        volume: 10,
        label: "draft".to_string(),
        cache_key: "keep-me".to_string(),
    };

    let memento: <DashboardState as Recallable>::Memento =
        serde_json::from_str(r#"{"volume":75,"label":"live"}"#).unwrap();

    dashboard.recall(memento);

    assert_eq!(dashboard.volume, 75);
    assert_eq!(dashboard.label, "live");
    assert_eq!(dashboard.cache_key, "keep-me");
}
```

`serde_json` is used here because it is easy to read in documentation.
For `no_std + serde` deployments, prefer a `no_std`-compatible format such as
`postcard`.

What happens here:

- `DashboardState` stays the runtime type
- the generated companion memento contains only `volume` and `label`
- the skipped field is preserved across recall
- the memento is named through `<DashboardState as Recallable>::Memento`

## Using `#[recallable_model]`

`#[recallable_model]` is the recommended entry point for the common case.

It always injects:

- `#[derive(Recallable, Recall)]`

With the default `serde` feature enabled, it also injects:

- `#[derive(serde::Serialize)]` on the source struct
- `#[serde(skip)]` on fields marked `#[recallable(skip)]`

Enum support is intentionally split:

- assignment-only enums can use `#[recallable_model]` directly
- enums with `PhantomData<_>` marker fields can also use it directly; those
  marker fields are auto-skipped, and explicit `#[recallable(skip)]` remains
  accepted
- enums with nested `#[recallable]` or other `#[recallable(skip)]` fields
  should derive `Recallable` and implement `Recall` or `TryRecall` manually

Example:

```rust
use recallable::recallable_model;

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
struct UserProfile {
    id: u64,
    display_name: String,
    #[recallable(skip)]
    cache_key: String,
}
```

### Attribute ordering requirement

`#[recallable_model]` must appear before the attributes it needs to inspect.

This is valid:

```rust
#[recallable_model]
#[derive(Clone, Debug)]
struct GoodOrder {
    value: u32,
}
```

This is not:

```rust
#[derive(serde::Serialize)]
#[recallable_model]
struct BadOrder {
    value: u32,
}
```

When `serde` is enabled, the macro injects `serde::Serialize` itself.
Placing a visible `Serialize` derive before the macro can therefore trigger a
duplicate-derive compile error.

### What gets generated

For a simple named struct, the expansion is conceptually equivalent to:

```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, Recallable, Recall)]
struct DashboardState {
    volume: u8,
    label: String,
    #[serde(skip)]
    #[recallable(skip)]
    cache_key: String,
}

#[derive(serde::Deserialize, Clone, Debug, PartialEq)]
struct DashboardStateMemento {
    volume: u8,
    label: String,
}

impl Recallable for DashboardState {
    type Memento = DashboardStateMemento;
}

impl Recall for DashboardState {
    fn recall(&mut self, memento: Self::Memento) {
        self.volume = memento.volume;
        self.label = memento.label;
    }
}
```

The exact generated name remains an implementation detail.
The intended way to refer to the type is `<Type as Recallable>::Memento`.

## Using direct derives

Use direct derives when you want explicit control over the source struct's
derives and serde behavior.

```rust
use recallable::{Recall, Recallable};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Recallable, Recall)]
struct SessionState {
    version: u32,
    #[serde(skip)]
    #[recallable(skip)]
    connection_id: u64,
}
```

Important distinction:

- `#[recallable_model]` mutates source-side serde behavior for the common case
- direct `#[derive(Recallable, Recall)]` does not

Direct derives are also the split point for complex enums:

- `#[derive(Recallable)]` supports enum-shaped mementos under the normal field rules
- `#[derive(Recall)]` works only for assignment-only enums
- enums with nested `#[recallable]` or skipped variant fields should derive
  `Recallable` and implement `Recall` or `TryRecall` manually

If you use direct derives and want the source struct to serialize in the same
shape as the generated memento, you must add the serde derives and
`#[serde(skip)]` attributes yourself.

## Skipped fields and memento visibility

Fields marked `#[recallable(skip)]` are:

- omitted from the generated memento
- left untouched when recall runs

This is what makes the crate useful for long-lived runtime objects with
non-state fields.

Generated mementos are intentionally somewhat opaque:

- refer to them as `<Type as Recallable>::Memento`
- expect compiler diagnostics to sometimes mention a concrete generated name
- expect the generated type name to remain an implementation detail
- expect the generated memento to use the same visibility as the source struct
- expect the generated memento fields themselves to remain private

This design pushes callers toward "construct or deserialize a memento, then
apply it" instead of depending on widened field visibility.

### Skipped `PhantomData` and retained generics

Most skipped fields simply disappear from the generated memento. `PhantomData<_>`
fields are auto-skipped by the derive, and the tricky case is when such a field
is the only field mentioning a generic that
still must remain part of the memento type.

```rust
use core::any::TypeId;
use core::marker::PhantomData;
use recallable::Recallable;

#[derive(Recallable)]
struct BoundDependent<T: From<U>, U> {
    value: T,
    #[recallable(skip)]
    marker: PhantomData<U>,
}

type Left = <BoundDependent<String, &'static str> as Recallable>::Memento;
type Right = <BoundDependent<String, String> as Recallable>::Memento;

assert_ne!(TypeId::of::<Left>(), TypeId::of::<Right>());
```

Why this needs a hidden marker:

- the skipped field means there is no visible memento field of type `U`
- `U` still matters, because the retained generic `T` depends on it through
  `T: From<U>`
- the generated memento type therefore needs to keep `U` alive internally

The derive handles that by synthesizing an internal `PhantomData` marker on the
generated memento.

If a skipped generic is otherwise unused, the derive prunes it instead of
preserving it:

```rust
use core::any::TypeId;
use core::marker::PhantomData;
use recallable::Recallable;

#[derive(Recallable)]
enum SkippedGenericEnum<T, U> {
    Value(T),
    Marker(#[recallable(skip)] PhantomData<U>),
}

type Left = <SkippedGenericEnum<u8, u16> as Recallable>::Memento;
type Right = <SkippedGenericEnum<u8, u32> as Recallable>::Memento;

assert_eq!(TypeId::of::<Left>(), TypeId::of::<Right>());
```

So the rule is:

- if a skipped generic is no longer needed, the memento drops it
- if it is still needed by retained generics or bounds, the derive keeps it via
  an internal hidden marker

## Recursive fields and container-defined semantics

Mark a field with `#[recallable]` when that field should use its own
`Recallable::Memento` and `Recall::recall` behavior instead of simple
assignment.

```rust
use recallable::{Recall, Recallable};

#[derive(Clone, Debug, Recallable, Recall)]
struct InnerCounter {
    value: u32,
}

#[derive(Clone, Debug, Recallable, Recall)]
struct Envelope<T> {
    payload: T,
    #[recallable]
    inner: InnerCounter,
    #[recallable(skip)]
    cache_label: String,
}
```

For `#[recallable]` fields, the macro does not impose one universal merge
strategy.
It delegates to the field type's own behavior.

That means container-like types can legitimately choose different memento shapes:

- `Self` for whole-value replacement
- `Option<T::Memento>` for selective inner updates
- `Vec<T::Memento>` for positional or zipped updates

This is a core design choice, not an accident.

## Fallible recall with `TryRecall`

Use `TryRecall` when applying a memento may fail validation.

```rust
use core::fmt;
use recallable::{Recallable, TryRecall};

struct Config {
    limit: u32,
}

#[derive(Clone)]
struct ConfigMemento {
    limit: u32,
}

#[derive(Debug)]
struct InvalidConfigError;

impl fmt::Display for InvalidConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "limit cannot be zero")
    }
}

impl core::error::Error for InvalidConfigError {}

impl Recallable for Config {
    type Memento = ConfigMemento;
}

impl TryRecall for Config {
    type Error = InvalidConfigError;

    fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error> {
        if memento.limit == 0 {
            return Err(InvalidConfigError);
        }
        self.limit = memento.limit;
        Ok(())
    }
}
```

There is intentionally no `#[derive(TryRecall)]`.
Fallible recall is where application-specific validation belongs.

Every `Recall` type automatically implements `TryRecall` with
`core::convert::Infallible`, so infallible models still fit APIs that expect
`TryRecall`.

## In-memory snapshots with `impl_from`

Enable `impl_from` when you want a derived `From<Type>` implementation for
the generated memento.

```toml
[dependencies]
recallable = { version = "0.2.0", features = ["impl_from"] }
```

```rust
use core::marker::PhantomData;
use recallable::{Recall, Recallable, recallable_model};

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct InnerState {
    value: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct DerivedEnvelope<T, K> {
    #[recallable]
    inner: T,
    version: u32,
    #[recallable(skip)]
    marker: PhantomData<K>,
}

fn main() {
    let original = DerivedEnvelope {
        inner: InnerState { value: 42 },
        version: 7,
        marker: PhantomData::<i32>,
    };

    let memento: <DerivedEnvelope<InnerState, i32> as Recallable>::Memento =
        original.clone().into();

    let mut target = DerivedEnvelope {
        inner: InnerState { value: 0 },
        version: 0,
        marker: PhantomData::<i32>,
    };

    target.recall(memento);
    assert_eq!(target, original);
}
```

For `#[recallable]` fields, this also requires:

```rust
<FieldType as Recallable>::Memento: From<FieldType>
```

That extra export-side bound is why `impl_from` is not enabled implicitly for
all workflows.

With `impl_from`, both struct and enum `Recallable` derives can generate
`From<Type>` for the companion memento, as long as the generated bounds hold.

## Manual trait implementations

You do not need the macros to use the traits.
Manual implementations work whether or not serde is enabled.
This is useful for explicit codebases, custom transport layers, or `no_std`
environments.

Disable default features only when you want recallable itself to stop enabling
serde support by default:

```toml
[dependencies]
recallable = { version = "0.2.0", default-features = false }
```

Then define the memento and recall behavior manually:

```rust
use recallable::{Recall, Recallable};

#[derive(Debug, PartialEq, Eq)]
struct EngineState {
    applied_ticks: u64,
    cached_checksum: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EngineMemento {
    applied_ticks: u64,
}

impl Recallable for EngineState {
    type Memento = EngineMemento;
}

impl Recall for EngineState {
    fn recall(&mut self, memento: Self::Memento) {
        self.applied_ticks = memento.applied_ticks;
    }
}
```

This manual style pairs naturally with:

- binary formats such as `postcard`
- fixed-capacity containers such as `heapless`
- transport layers that do not use serde

## Supported shapes, generics, and lifetimes

The derive macros support more than just simple named structs.

### Supported item shapes

- named structs
- tuple structs
- unit structs
- enums for `Recallable`
- enums for `Recall` and `recallable_model` only when every variant field is
  assignment-only
- complex enums should derive `Recallable` only and supply manual `Recall` or
  `TryRecall`

### Supported generic forms

- type generics
- const generics
- associated types
- path types such as `nested::Inner` or `<T as Trait>::Assoc`

Generated mementos retain only the generics and bounds actually needed by the
non-skipped state fields.

### Lifetime support

Lifetime parameters are supported only when the generated memento can remain an
owned type.

That means:

- skipped borrowed fields are allowed
- `PhantomData<_>` fields are allowed because the derive auto-skips them; this
  includes lifetime-bearing markers such as `PhantomData<&'a T>`
- non-skipped borrowed state fields like `&'a str` are rejected

## Serialization guidance

Recallable is codec-agnostic.
It only cares that your chosen format can:

- serialize the source-side state you emit
- deserialize into the memento type you apply

Practical guidance:

- use `serde_json` for readable examples, tests, and debugging
- use `postcard` or another binary format when you care about size or `no_std`
- use `#[recallable_model]` when you want the source struct's serialized shape
  to align automatically with the generated memento
- use direct derives when you want explicit source-side serde control

Important asymmetry:

- generated mementos derive `Deserialize`
- generated mementos do not derive `Serialize`

That is intentional.
The crate is designed around applying mementos, not treating them as the public
write-side output format by default.

## Macro and trait reference

### `#[recallable_model]`

Convenience attribute for the common struct or assignment-only enum model path.
It is the recommended default whether or not `serde` is enabled; with `serde`
enabled it also removes extra derive boilerplate.

Behavior:

- injects `Recallable` and `Recall`
- injects `serde::Serialize` when the `serde` feature is enabled
- injects `#[serde(skip)]` onto fields marked `#[recallable(skip)]`
- rejects complex enums where generated `Recall` would be ambiguous; those
  should derive `Recallable` and implement `Recall` or `TryRecall` manually

### `#[derive(Recallable)]`

Generates:

- the companion memento type
- the `Recallable` implementation
- `From<Type>` for the memento when `impl_from` is enabled
- enum-shaped mementos for enums, even when `Recall` must stay manual

### `#[derive(Recall)]`

Generates the `Recall` implementation.

Behavior:

- struct fields are handled as before
- enum derives are supported only for assignment-only variants, plus
  `PhantomData<_>` marker fields that are auto-skipped by the derive
- enums with nested `#[recallable]` or other skipped fields should derive
  `Recallable` and implement `Recall` or `TryRecall` manually

### `#[recallable]`

Marks a field for recursive recall using the field type's own memento and
recall behavior.

### `#[recallable(skip)]`

Omits a field from the generated memento and preserves the field during recall.

### `#[recallable(skip_memento_default_derives)]`

Suppresses the generated `Clone`, `Debug`, and `PartialEq` derives and their
bounds on the memento type.
With `serde` enabled, `Deserialize` is still derived.

### `Recallable`

```rust
pub trait Recallable {
    type Memento;
}
```

### `Recall`

```rust
pub trait Recall: Recallable {
    fn recall(&mut self, memento: Self::Memento);
}
```

### `TryRecall`

```rust
pub trait TryRecall: Recallable {
    type Error: core::error::Error + Send + Sync + 'static;
    fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error>;
}
```

## Design guarantees

- Derive macros target ordinary data models, not arbitrary Rust items
- `Recallable` stays apply-side only and does not define a universal export API
- Generated mementos derive `Clone`, `Debug`, and `PartialEq` by default
- With `serde` enabled, generated mementos also derive `serde::Deserialize`
- Generated memento fields remain private
- Memento shape for `#[recallable]` fields is delegated to the field type
- `TryRecall` is automatically implemented for all `Recall` types with
  `Infallible`

## Current limitations

- `#[derive(Recallable)]` supports enums under the normal field rules
- `#[derive(Recall)]` and `#[recallable_model]` support enums only for
  assignment-only variants
- complex enums should derive `Recallable` and implement `Recall` or
  `TryRecall` manually
- Borrowed non-skipped state fields are rejected
- `#[recallable]` is path-only and does not accept tuple/reference/slice/function
  syntax directly
- Serde attributes are not forwarded to the generated memento
- If you need custom serde behavior on the memento itself, define the memento
  manually and implement `Recallable` and `Recall` yourself

## Examples and project files

Runnable examples live under `recallable/examples/`:

```bash
cargo run -p recallable --example basic_model
cargo run -p recallable --example nested_generic
cargo run -p recallable --example postcard_roundtrip
cargo run -p recallable --no-default-features --example manual_no_serde
cargo run -p recallable --no-default-features --features impl_from --example impl_from_roundtrip
```

Useful repository files:

- `recallable/examples/basic_model.rs`
- `recallable/examples/nested_generic.rs`
- `recallable/examples/postcard_roundtrip.rs`
- `recallable/examples/manual_no_serde.rs`
- `recallable/examples/impl_from_roundtrip.rs`
- `CONTRIBUTING.md`
- `CHANGELOG.md`

## Contributing, license, and changelog

- Contribution guide: [CONTRIBUTING.md](CONTRIBUTING.md)
- License: [LICENSE-MIT.txt](LICENSE-MIT.txt) or [LICENSE-APACHE.txt](LICENSE-APACHE.txt)
- Release notes: [CHANGELOG.md](CHANGELOG.md)
