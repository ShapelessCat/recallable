# Recallable

[![CI](https://github.com/ShapelessCat/recallable/actions/workflows/ci.yaml/badge.svg)](https://github.com/ShapelessCat/recallable/actions/workflows/ci.yaml)
[![Crates.io](https://img.shields.io/crates/v/recallable.svg)](https://crates.io/crates/recallable)
[![Documentation](https://docs.rs/recallable/badge.svg)](https://docs.rs/recallable)
[![recallable MSRV](https://img.shields.io/crates/msrv/recallable.svg?label=recallable%20msrv&color=lightgray)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0.html)
[![recallable-macro MSRV](https://img.shields.io/crates/msrv/recallable-macro.svg?label=recallable-macro%20msrv&color=lightgray)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Traits (`Recallable`, `Recall`, `TryRecall`) and procedural macros for defining Memento pattern types and their state
restoration behaviors.

Implementing `Recallable` for a struct means specifying its associated memento type. Its subtraits `Recall` and
`TryRecall` provide the interface for applying state restoration infallibly and fallibly, respectively. The blanket
`TryRecall` implementation for all `Recall` types mirrors the `From`/`TryFrom` relationship in the standard library, so
infallible types work seamlessly in fallible contexts.

Note:
Each `Recallable` implementation chooses one associated memento type. For container-like types,
different implementations may reasonably choose different memento shapes and recall behavior.

## Why Recallable?

Recallable shines when you need to persist and update state without hand-maintaining parallel state structs. A common
use case is durable execution: save only true state while skipping non-state fields (caches, handles, closures), then
restore or update state incrementally.

Typical scenarios include:

- Durable or event-sourced systems where only state fields should be persisted.
- Streaming or real-time pipelines that receive incremental updates.
- Syncing or transporting partial state over the network.

The provided procedural macros handle the heavy lifting; they generate companion memento types and recall logic. See
[Features](#features) and [How It Works](#how-it-works) for details.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Runnable Examples](#runnable-examples)
- [Requirements & Limitations](#requirements--limitations)
- [Usage](#usage)
  - [Basic Example](#basic-example)
  - [Using `#[recallable_model]`](#using-recallable_model)
  - [Skipping Fields](#skipping-fields)
  - [Nested Recallable Structs](#nested-recallable-structs)
  - [Fallible Recalling](#fallible-recalling)
- [How It Works](#how-it-works)
- [API Reference](#api-reference)
- [Contributing](#contributing)
- [License](#license)

## Features

- **`#[recallable_model]` Attribute Macro**: Injects `#[derive(Recallable, Recall)]` and, with the default `serde`
  feature, `#[derive(serde::Serialize)]` plus `#[serde(skip)]` on fields marked `#[recallable(skip)]`
- **Automatic Memento Type Generation**: Derives a companion memento type for any struct annotated with
  `#[derive(Recallable)]`, exposed as `<Type as Recallable>::Memento`. The generated type is kept out of your namespace,
  preventing pollution and allowing internal naming to evolve without breaking downstream code
- **Recursive Recalling**: Use the `#[recallable]` attribute to mark fields that require recursive recalling
- **Smart Exclusion**: Excludes fields marked with `#[recallable(skip)]`
- **Serde Integration (optional, default)**: Generated memento types automatically implement `serde::Deserialize`
  but not `Serialize` — mementos are meant to be received and applied, not sent back out. This asymmetry aligns with
  typical durable-execution and incremental-update use cases. Exclude the `serde` feature to opt out
- **Generic Support**: Supports type and const generics, preserves retained bounds/defaults and relevant `where`
  predicates on generated mementos, and infers trait bounds for bare `T` `#[recallable]` fields
- **Optional `From` Derive**: Enable `From<Struct>` for `<Struct as Recallable>::Memento` with the `impl_from`
  feature. For `#[recallable]` fields, this also requires the field's memento type to implement
  `From<FieldType>`
- **Zero Runtime Overhead**: All code generation happens at compile time
- **`no_std` Support**: Compatible with `no_std` environments (for example, with `postcard` + `heapless`)

## Installation

**MSRV:** Rust 1.88 (edition 2024). CI validates both the current stable toolchain and Rust 1.88.0.

Add this to your `Cargo.toml`:

```toml
[dependencies]
recallable = "0.1.0" # Please use the latest version
```

Check this project's Cargo feature flags to see what you want to enable or disable.

The examples in this README also use `serde`, `postcard`, and `heapless`. Add them as dependencies if you want to run
the examples:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
postcard = "1"
heapless = "0.9.2"
```

## Runnable Examples

The repository includes runnable example binaries under `recallable/examples/`.
Because the workspace root uses a virtual manifest, run them with `-p recallable`:

```bash
cargo run -p recallable --example basic_model
cargo run -p recallable --example nested_generic
cargo run -p recallable --example postcard_roundtrip
cargo run -p recallable --no-default-features --example manual_no_serde
cargo run -p recallable --no-default-features --features impl_from --example impl_from_roundtrip
```

## Requirements & Limitations

Before diving into the examples, be aware of the following constraints:

- **Structs only** — enums and unions are not supported.
- **Lifetime parameters are allowed only when the generated memento can stay owned** — structs may have lifetime
  parameters, but any non-skipped field that borrows one of the struct's lifetimes (for example `&'a str` or
  `Vec<&'a u8>`) is rejected. Skipped borrowed fields and lifetime-only markers such as `PhantomData<&'a T>` are
  supported because they do not need to appear in the generated memento.
- **`#[recallable]` is limited to path types** — bare type parameters (`T`), parameterized paths like `Option<T>` and
  `HashMap<K, V>`, module-qualified paths, associated types like `T::Assoc`, and qualified associated types like
  `<T as Trait>::Assoc` are supported. Non-path types such as tuples, arrays, references, slices, and function types
  are still rejected.
- **Implicit trait requirements on field types** — the generated memento struct derives `Clone`, `Debug`, and
  `PartialEq` (and `Deserialize` when the `serde` feature is enabled). For regular fields, the field type itself must
  implement these traits. For `#[recallable]` fields, it is the field's *memento type*
  (`<FieldType as Recallable>::Memento`) that must implement them. If any required trait is missing, compilation fails
  with an error pointing at generated code.
- **`impl_from` adds nested `From` requirements** — when the `impl_from` feature is enabled, the derived
  `From<Struct>` implementation converts each `#[recallable]` field with `Into`, so
  `<FieldType as Recallable>::Memento` must implement `From<FieldType>`. Some otherwise valid generic container
  designs cannot satisfy this because of Rust coherence rules.
- **`#[recallable_model]` attribute ordering** — `#[recallable_model]` must appear *before* any attributes it needs
  to inspect (e.g., before `#[derive(Serialize)]`). Attribute macros only see attributes that follow them in source
  order.
- **Serde behavior** — with the default `serde` feature:
  - `#[recallable_model]` injects `#[derive(serde::Serialize)]` and adds `#[serde(skip)]` to `#[recallable(skip)]`
    fields. Adding a manual `#[derive(Serialize)]` is a compile error.
  - Direct `#[derive(Recallable, Recall)]` does **not** inject `serde::Serialize` or add `#[serde(skip)]` to skipped
    fields on the source struct. If you want serialization to match recalling behavior when using direct derives, add
    those serde attributes yourself.
  - `#[derive(Recallable)]` makes the memento derive `Deserialize` but not `Serialize`
    (see [Serde Integration](#features) above for the rationale).
  - Serde attributes like `#[serde(rename = "...")]` on the original struct are not forwarded to the memento struct.
    The generated memento mirrors the original struct's field layout by design — the macro intentionally keeps
    generation simple rather than adding complex attribute forwarding. For use cases requiring custom serde attributes
    on the memento, implement `Recallable` and define the memento struct manually.

## Usage

### Basic Example

```rust
use recallable::{Recall, Recallable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Recallable, Recall)]
struct User {
    id: u64,
    name: String,
    email: String,
}

fn main() {
    let mut user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    // Serialize the current state
    let state_bytes = postcard::to_vec::<_, 128>(&user).unwrap();

    // Deserialize into a memento
    let memento: <User as Recallable>::Memento = postcard::from_bytes(&state_bytes).unwrap();

    let mut default = User::default();
    // Apply the memento
    default.recall(memento);

    assert_eq!(default, user);
}
```

### Using `#[recallable_model]`

The simplest way to use this library is the attribute macro:

```rust
use recallable::recallable_model;

#[recallable_model]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct User {
    id: u64,
    name: String,
    #[recallable(skip)]
    cache_key: String,
}
```

`#[recallable_model]` always adds `Recallable` and `Recall` derives. With the default
`serde` feature enabled, it also adds `serde::Serialize` and injects `#[serde(skip)]`
for fields marked `#[recallable(skip)]`.
Add any other derives you need (for example, `Deserialize`) alongside it.

**Note:** `#[recallable_model]` must appear before other derive/attribute macros it needs
to interact with. See [Requirements & Limitations](#requirements--limitations) for details.

### Skipping Fields

Fields can be excluded from recalling using `#[recallable(skip)]`:

```rust
use recallable::recallable_model;
use serde::Deserialize;

#[recallable_model]
#[derive(Clone, Debug, Deserialize)]
struct Measurement<T, F> {
    value: T,
    #[recallable(skip)]
    compute_fn: F,
}
```

Fields marked with `#[recallable(skip)]` are excluded from the generated memento type. The generated companion struct is
an implementation detail; the supported way to refer to it is `<Type as Recallable>::Memento`. If you use
`#[recallable_model]` with the default `serde` feature enabled, those fields also receive `#[serde(skip)]` so serialized
state and mementos stay aligned. If you derive `Recallable`/`Recall` directly, add `#[serde(skip)]` yourself when you
want serialization to match recalling behavior.

### Nested Recallable Structs

The macros support retained type and const generics on both the source type and generated memento:

```rust
use recallable::{Recall, Recallable};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Recallable, Recall)]
struct Container<Closure> {
    #[serde(skip)]
    #[recallable(skip)]
    computation_logic: Closure, // Not a part of state
    metadata: String,
}

#[derive(Clone, Debug, Serialize, Recallable, Recall)]
struct Wrapper<T, Closure, const N: usize> {
    data: T,
    #[recallable]
    inner: ConstContainer<Closure, N>,
}

#[derive(Clone, Debug, Serialize, Recallable, Recall)]
struct ConstContainer<Closure, const N: usize> {
    #[serde(skip)]
    #[recallable(skip)]
    computation_logic: Closure,
    version: u32,
}
```

The macros automatically:

- Use whatever `Recallable::Memento` and `Recall::recall` implementation each `#[recallable]` field type provides.
  For container-like field types, this means the macro does not prescribe one canonical merge strategy: a field type
  may replace itself wholesale, selectively update inner values, or apply some other domain-specific behavior.
- Preserve only the generic parameters used by non-skipped fields, plus any dependencies pulled in by retained bounds,
  defaults, or relevant `where` predicates
- Keep retained bounds/defaults and filtered `where` clauses on the generated memento
- Add appropriate trait bounds (`Recallable`, `Recall`) based on field usage
- Generate correctly parameterized memento types

### Fallible Recalling

The `TryRecall` trait allows for fallible updates, which is useful when memento application requires validation:

```rust
use recallable::{TryRecall, Recallable};
use core::fmt;

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

## How It Works

When you derive `Recallable` on a struct, for instance, `Struct`:

1. **Companion Memento Type**: The macro generates an internal companion memento struct and exposes
   it as `<Struct as Recallable>::Memento`. That type mirrors the original structure but only
   includes fields that are part of the memento. Here are the rules:
   - Each field marked with `#[recallable]` in `Struct` is typed with
     `<FieldType as Recallable>::Memento` in the generated companion type. The macro does not
     special-case container types; it uses whatever memento shape the field type defines.
   - Fields marked with `#[recallable(skip)]` are excluded.
   - The left fields are copied directly with their original types.
   - The generated memento type uses the same visibility as `Struct`, but its fields are always
     generated without visibility modifiers and therefore remain private to the containing module.
     This is intentional: mementos are expected to be created and consumed alongside the companion
     struct, primarily through `Recall::recall` and `TryRecall::try_recall`, not as a public
     field-inspection surface.

2. **Trait Implementation**: The macro implements `Recallable` for `Struct` and sets
   `type Memento` to that generated companion type (see the API reference for the exact trait
   definition).

3. **Serialized State to Recall**: If you serialize a `Struct` instance, that serialized value can
   be deserialized into `<Struct as Recallable>::Memento`, which yields a memento representing the
   serialized state.

When you derive `Recall` on a struct:

1. **Recall Method**: The `recall` method updates the struct:
   - Regular fields are directly assigned from the memento
   - `#[recallable]` fields are recursively recalled via their own `recall` method, so field-level
     replace/merge behavior comes from the field type's implementation

2. **Trait Implementation**: The macro generates `Recall` implementation for the target struct (see
API reference for the exact trait definitions).

## API Reference

### `#[recallable_model]`

Attribute macro that injects `Recallable` and `Recall` derives for a struct.

**Behavior:**

- Adds `#[derive(Recallable, Recall)]` to the target struct.
- With the default `serde` feature enabled, it also derives `serde::Serialize` and
  applies `#[serde(skip)]` to fields annotated with `#[recallable(skip)]`.
- Direct `#[derive(Recallable, Recall)]` does not mutate the source struct's serde behavior in those ways.

**Attribute ordering:** `#[recallable_model]` must appear before any attributes it needs
to inspect. An attribute macro's input only contains attributes that follow it in source
order.

### `#[derive(Recallable)]`

Generates a companion memento type, exposed as `<Struct as Recallable>::Memento`, and implements
`Recallable` for a struct.

**Requirements:**

- Must be applied to a struct (not enums or unions)
- Supports lifetime-parameterized structs only when all non-skipped state fields remain owned in the generated
  memento; non-skipped borrowed fields are rejected
- Works with named, unnamed (tuple), and unit structs

The generated memento struct derives `Clone`, `Debug`, and `PartialEq`. With the `serde`
feature enabled, it also derives `Deserialize`. For regular fields, the field type must
implement these traits. For `#[recallable]` fields, the field's memento type
(`<FieldType as Recallable>::Memento`) must implement them.

The generated memento type uses the same visibility as the source struct. Its fields always remain
private because the macro does not emit field-level visibility modifiers. This is intentional: the
supported usage is to pass the value around as `<Struct as Recallable>::Memento`, typically into
`Recall::recall` or `TryRecall::try_recall`, rather than to rely on field-by-field inspection
across module boundaries.

Generated mementos retain the source generics that are actually needed by non-skipped fields,
including const parameters, along with any retained bounds/defaults and filtered `where`
predicates required to keep the memento well-formed.

When the `impl_from` feature is enabled, the generated `From<Struct>` implementation also requires
`<FieldType as Recallable>::Memento: From<FieldType>` for each `#[recallable]` field.

### `#[derive(Recall)]`

Derives the `Recall` trait implementation for a struct.

**Requirements:**

- Must be applied to a struct (not enums or unions)
- Supports lifetime-parameterized structs only when all non-skipped state fields remain owned in the generated
  memento; non-skipped borrowed fields are rejected
- Works with named, unnamed (tuple), and unit structs
- The target type must implement `Recallable` (derive it or implement manually)

### `#[recallable]` Attribute

Marks a field for recursive recalling.

**Requirements:**

- For `#[derive(Recallable)]`, the field type must implement `Recallable`
- For `#[derive(Recall)]`, the field type must implement `Recall`
- Accepts any path type, including parameterized paths (`Option<T>`, `HashMap<K, V>`),
  module-qualified paths, and associated types (`T::Assoc`, `<T as Trait>::Assoc`)
- Non-path types such as tuples, arrays, references, slices, and function types are not yet
  supported with `#[recallable]`

`#[recallable]` does not impose one canonical container semantics. It simply uses the field
type's own `Recallable::Memento` and `Recall::recall` implementations.

### `Recallable` Trait

```rust
pub trait Recallable {
    type Memento;
}
```

- `Memento`: The associated memento type. When `#[derive(Recallable)]` is applied, the generated
  companion struct is an implementation detail; refer its type with `<Type as Recallable>::Memento`.
  For container-like types, this associated type is intentionally application-defined rather than
  fixed by the crate.

### `Recall` Trait

```rust
pub trait Recall: Recallable {
    fn recall(&mut self, memento: Self::Memento);
}
```

- `recall`: Method to apply a memento to the current instance

### `TryRecall` Trait

A fallible variant of `Recall` for cases where applying a memento might fail.

```rust
pub trait TryRecall: Recallable {
    type Error: core::error::Error + Send + Sync + 'static;
    fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error>;
}
```

- `try_recall`: Applies the memento, returning a `Result`. A blanket implementation exists for all types that implement
  `Recall` (where `Error` is `core::convert::Infallible`).

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to get started.

## License

This project is licensed under the [MIT License](LICENSE-MIT.txt) and [Apache-2.0 License](LICENSE-APACHE.txt).

## Related Projects

- [serde](https://serde.rs/) - Serialization framework that integrates seamlessly with Recallable

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.
