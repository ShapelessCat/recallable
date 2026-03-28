# Recallable

[![CI](https://github.com/ShapelessCat/recallable/actions/workflows/ci.yaml/badge.svg)](https://github.com/ShapelessCat/recallable/actions/workflows/ci.yaml)
[![Crates.io](https://img.shields.io/crates/v/recallable.svg)](https://crates.io/crates/recallable)
[![Documentation](https://docs.rs/recallable/badge.svg)](https://docs.rs/recallable)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`recallable` is a `no_std`-friendly crate for Memento-pattern state recovery in Rust.
It gives a type a companion memento type and a way to apply that memento to an
already initialized value.

This is useful when your runtime struct contains a mix of:

- durable state that should be restored or updated
- runtime-only fields that must stay alive across recall

Typical examples are caches, handles, closures, connection ids, metrics state,
or other non-persisted fields that should not be reconstructed from serialized
input.

## What It Provides

- `Recallable`: declares a companion `Memento` type
- `Recall`: applies that memento infallibly
- `TryRecall`: applies it fallibly with validation
- `#[derive(Recallable)]`: generates the companion memento type
- `#[derive(Recall)]`: generates recall logic for structs and assignment-only enums
- `#[recallable_model]`: convenience attribute for the common struct or assignment-only enum path; with
  `serde` enabled it also removes extra derive boilerplate

The crate intentionally does not force one universal memento shape for
container-like field types. A field type can choose whole-value replacement,
selective inner updates, zipped updates, or any other domain-specific behavior
through its own `Recallable::Memento` and `Recall::recall` implementations.

## Why Not Just Deserialize?

Normal `Deserialize` constructs a brand-new value.
That is awkward when you already have a live runtime object with fields that
should survive updates unchanged.

`Recallable` lets you deserialize or construct a memento and apply it to an
existing value instead:

- durable state changes
- skipped runtime fields stay untouched
- nested `#[recallable]` fields use their own recall behavior

## Quickstart

With the default `serde` feature, the easiest path is `#[recallable_model]`.

```toml
[dependencies]
recallable = "0.2"
serde_json = "1"
```

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

`#[recallable_model]` injects:

- `#[derive(Recallable, Recall)]`
- `#[derive(serde::Serialize)]` when the default `serde` feature is enabled
- `#[serde(skip)]` for fields marked `#[recallable(skip)]`

For enums, `#[recallable_model]` is intentionally narrower than `#[derive(Recallable)]`:

- assignment-only enums are supported directly
- enums with skipped `PhantomData<_>` marker fields are still supported directly
- enums with nested `#[recallable]` or other `#[recallable(skip)]` fields should
  derive `Recallable` and implement `Recall` or `TryRecall` manually

## Features

- `serde` (default): enables macro-generated serde support; generated mementos derive
  `serde::Deserialize`, and `#[recallable_model]` also adds the source-side serde behavior
  described above. This feature remains compatible with `no_std` as long as your serde stack is
  configured for `no_std`.
- `impl_from`: generates `From<Struct>` for `<Struct as Recallable>::Memento`
- `full`: convenience feature for `serde` + `impl_from`
- `default-features = false`: disables recallable's default serde integration. It is useful for
  non-serde setups, but it is not what makes `no_std` possible.

Example dependency sets:

```toml
[dependencies]
# Readable std example
recallable = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```toml
[dependencies]
# no_std + serde example
recallable = { version = "0.2", default-features = false, features = ["serde"] }
serde = { version = "1", default-features = false, features = ["derive"] }
postcard = { version = "1", default-features = false, features = ["heapless"] }
heapless = { version = "0.9.2", default-features = false }
```

```toml
[dependencies]
# In-memory snapshots
recallable = { version = "0.2", features = ["impl_from"] }
```

## Two Common Workflows

### Persistence and restore

This is the default path when state crosses process boundaries or is written to
disk. It works in both `std` and `no_std` environments; only the serialization
format and serde configuration differ.

1. Serialize the source struct.
2. Deserialize into `<Type as Recallable>::Memento`.
3. Apply the memento with `recall` or `try_recall`.

This flow is especially convenient with `#[recallable_model]`, because the
source struct's serialized shape already matches the generated memento shape.

### In-memory snapshots

Enable `impl_from` when you want an owned memento inside the same process for
checkpoint/rollback, undo stacks, or test fixtures.

```rust
use recallable::{Recall, Recallable, recallable_model};

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct Counter {
    value: i32,
}

fn main() {
    let original = Counter { value: 42 };
    let memento: <Counter as Recallable>::Memento = original.clone().into();

    let mut target = Counter { value: 0 };
    target.recall(memento);

    assert_eq!(target, original);
}
```

## Manual trait implementations

You do not need the macros to use the traits.
Manual implementations work whether or not serde is enabled.

Disable default features only when you want recallable itself to stop enabling serde support by
default:

```toml
[dependencies]
recallable = { version = "0.2", default-features = false }
```

Define the memento type and recall behavior manually:

```rust
use recallable::{Recall, Recallable};

struct EngineState {
    applied_ticks: u64,
    cached_checksum: u64,
}

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

## Important Notes

- `#[recallable_model]` must appear before the attributes it needs to inspect.
- Direct `#[derive(Recallable, Recall)]` does not modify source serde behavior.
  If you need `Serialize` or `#[serde(skip)]`, add them yourself.
- There is intentionally no `#[derive(TryRecall)]`; fallible recall is where
  application-specific validation belongs.
- Generated mementos are meant to be named through `<Type as Recallable>::Memento`.
- Generated memento fields remain private.

## Current Limitations

- Derive macros support structs and enums
- `#[derive(Recallable)]` supports enums under the normal field rules
- `#[derive(Recall)]` and `#[recallable_model]` support enums only when every
  non-marker variant field is assignment-only
- Enums with skipped `PhantomData<_>` marker fields are still supported
- Enums with nested `#[recallable]` or other `#[recallable(skip)]` fields
  should derive `Recallable` and implement `Recall` or `TryRecall` manually
- Borrowed state fields are rejected unless they are skipped
- `#[recallable]` is path-only: it supports type parameters, path types, and
  associated types, but not tuple/reference/slice/function syntax directly
- Serde attributes are not forwarded to the generated memento

## Examples

Runnable examples live under `recallable/examples/`:

```bash
cargo run -p recallable --example basic_model
cargo run -p recallable --example nested_generic
cargo run -p recallable --example postcard_roundtrip
cargo run -p recallable --no-default-features --example manual_no_serde
cargo run -p recallable --no-default-features --features impl_from --example impl_from_roundtrip
```

## More Documentation

- Full guide and reference: [GUIDE.md](GUIDE.md)
- API docs: [docs.rs/recallable](https://docs.rs/recallable)
- Contribution guide: [CONTRIBUTING.md](CONTRIBUTING.md)
- Changelog: [CHANGELOG.md](CHANGELOG.md)

## License

Licensed under either [MIT](LICENSE-MIT.txt) or [Apache-2.0](LICENSE-APACHE.txt),
at your option.
