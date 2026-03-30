# Serde Attribute Forwarding to Generated Mementos

## Problem

When a source struct uses `#[serde(rename = "...")]` or `#[serde(alias = "...")]`, the
generated memento type does not inherit those attributes. Since the primary workflow is
`serialize source → deserialize memento → recall`, this creates a wire-compatibility gap:
the serialized form uses the renamed key, but the memento's `Deserialize` impl expects the
original Rust field name.

## Scope

**Supported attributes:** `rename` (single value) and `alias` (multiple values).

**Not supported:** `default`, `rename_all`, `deserialize_with`, `flatten`, `borrow`, or any
other serde attributes. These may be added in future iterations.

## Design

### New module: `serde_attrs`

A new module directory at `recallable-macro/src/context/internal/serde_attrs/` that runs as a
**separate analysis pass** over the `DeriveInput`, independent from the existing
`FieldIr` / `StructIr` / `EnumIr` pipeline. A directory (not a single file) to accommodate
future attribute support.

This module is responsible for:

1. Extracting `rename` and `alias` from `#[recallable(...)]` and `#[serde(...)]` attributes
2. Merging values from both sources (for manual `#[derive(Recallable)]`)
3. Detecting conflicts and illegal usage
4. Producing a result that codegen consumes to emit `#[serde(...)]` on memento fields

### Data model

```rust
/// Parsed serde-relevant attributes for a single field.
pub(crate) struct SerdeFieldAttrs {
    pub(crate) rename: Option<syn::LitStr>,
    pub(crate) aliases: Vec<syn::LitStr>,
}

/// Result of the serde attribute analysis pass for a struct.
pub(crate) struct SerdeStructAttrs {
    /// Per-field attrs, indexed by field position.
    pub(crate) fields: Vec<SerdeFieldAttrs>,
}

/// Result of the serde attribute analysis pass for an enum.
pub(crate) struct SerdeEnumAttrs {
    /// Per-variant, per-field attrs.
    pub(crate) variants: Vec<Vec<SerdeFieldAttrs>>,
}
```

`SerdeFieldAttrs` provides a `fn to_memento_tokens(&self) -> TokenStream2` method that
produces the `#[serde(rename = "...")]` and `#[serde(alias = "...")]` attribute tokens
(or empty if no attrs are present).

### Two macro entry points, two behaviors

#### `#[recallable_model]` (attribute macro)

- Parses `#[recallable(rename = "...", alias = "...")]` from fields
- **Rejects** manual `#[serde(rename = "...")]` or `#[serde(alias = "...")]` on any field
  with a compile error:

  ```text
  `#[recallable_model]` manages serde attributes automatically;
  use `#[recallable(rename = "...")]` instead of `#[serde(rename = "...")]`
  ```

- Inserts `#[serde(rename = "...")]` / `#[serde(alias = "...")]` on the **source** field
  (similar to existing `add_serde_skip_attrs`)
- The same `SerdeAttrs` result feeds into memento codegen to emit matching attributes on
  memento fields

#### `#[derive(Recallable)]` (derive macro, manual)

- Parses both `#[recallable(rename = "...", alias = "...")]` and
  `#[serde(rename = "...", alias = "...")]` from fields
- **Merges** values from both sources:
  - `#[serde(rename = "x")]` alone → implicitly treated as `#[recallable(rename = "x")]`
  - `#[recallable(rename = "x")]` alone → valid
  - Both present with same value → valid
  - Both present with different values → compile error:

    ```text
    conflicting `rename` values: `#[serde(rename = "x")]` and
    `#[recallable(rename = "y")]` must match
    ```

- For `alias`: values from both sources are unioned (deduplicated by string value).
  No conflict is possible since aliases are additive.
- The merged result feeds into memento codegen

### Interaction with `#[recallable(skip)]`

`#[recallable(rename = "...")]` or `#[recallable(alias = "...")]` on a
`#[recallable(skip)]` field is a compile error. Skipped fields do not appear in the
memento, so wire-format attributes on them are meaningless.

```text
`rename` and `alias` cannot be used on a `#[recallable(skip)]` field
```

### Serde feature gate

`#[recallable(rename = "...")]` and `#[recallable(alias = "...")]` require the `serde`
feature to be enabled. When serde is disabled, these produce a compile error:

```text
`#[recallable(rename = "...")]` requires the `serde` feature
```

### `#[recallable(...)]` attribute parsing changes

`determine_field_behavior` in `fields.rs` currently rejects anything other than `skip`
inside `#[recallable(...)]`. It needs to accept `rename` and `alias` as known parameters
without rejecting them — but the actual value extraction happens in the `serde_attrs` module.

### Codegen integration

#### Call flow

```text
lib.rs (derive Recallable)
  → analyze_item(input)             → ItemIr
  → analyze_serde_attrs(input)      → SerdeAttrs  (new)
  → gen_memento_type(ir, env, serde_attrs)

lib.rs (recallable_model)
  → model_macro::expand
    → analyze_serde_attrs(input)    → SerdeAttrs  (new, for rejection + source annotation)
    → reject manual #[serde(rename/alias)]
    → add_serde_forwarded_attrs_to_fields (inserts #[serde] on source)
    → (derive expansion picks up SerdeAttrs again for memento)
```

#### Memento field emission

`gen_memento_struct` and `gen_memento_enum` gain a `serde_attrs` parameter. When iterating
fields to build the memento body, each field's `SerdeFieldAttrs::to_memento_tokens()` is
prepended before the existing field tokens from `build_memento_field_tokens`.

For a field with `#[recallable(rename = "x", alias = "a", alias = "b")]`, the memento emits:

```rust
#[serde(rename = "x")]
#[serde(alias = "a")]
#[serde(alias = "b")]
field_name: FieldType
```

#### `model_macro.rs` source annotation

A new function `add_serde_forwarded_attrs_to_fields` (parallel to existing
`add_serde_skip_attrs_to_fields`) iterates fields and inserts `#[serde(rename = "...")]`
and `#[serde(alias = "...")]` on the source struct fields based on `#[recallable(...)]`
attributes.

## Testing

### Unit tests in `serde_attrs` module

- Parse `#[recallable(rename = "x")]` → correct `SerdeFieldAttrs`
- Parse `#[recallable(alias = "a", alias = "b")]` → collects both aliases
- Parse `#[serde(rename = "x")]` alone (manual derive) → implicit recallable rename
- Both present with same value → merges without error
- Both present with conflicting values → compile error
- `#[recallable(rename)]` without serde feature → compile error
- Mixed: `#[recallable(rename = "x", alias = "a")]` → both fields populated
- `#[recallable(rename = "x")]` on a `#[recallable(skip)]` field → compile error

### Unit tests in `model_macro.rs`

- `#[recallable_model]` with `#[serde(rename = "x")]` on a field → rejected
- `#[recallable_model]` with `#[serde(alias = "x")]` on a field → rejected
- `#[recallable_model]` with `#[recallable(rename = "x")]` → source field gets `#[serde(rename = "x")]`

### Integration tests in `recallable/tests/`

- Struct with `#[recallable(rename = "x")]` round-trips through JSON:
  serialize source → deserialize memento → recall → values match
- Struct with `#[recallable(alias = "old_name")]` deserializes memento from JSON using alias key
- Enum variant fields with rename/alias round-trip correctly

### UI compile-fail tests in `recallable/tests/ui/`

- Conflicting rename values between `#[serde]` and `#[recallable]`
- Manual `#[serde(rename)]` under `#[recallable_model]`
- `#[recallable(rename)]` without serde feature
- `#[recallable(rename)]` on a `#[recallable(skip)]` field

## Non-goals

- Container-level attributes (`rename_all`, `deny_unknown_fields`)
- `#[serde(default)]` / `#[serde(default = "path")]`
- Variant-level serde attributes
- Moving existing `skip` logic into `serde_attrs` — `skip` is a field strategy concern,
  not a wire-format concern
