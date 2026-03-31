# Serde Validation Responsibility Refactor

**Goal:** Move all serde attribute validation into `#[derive(Recallable)]`, make `#[recallable_model]` a thin orchestrator, and accumulate validation errors instead of bailing on the first one.

**Motivation:** The current code scatters serde-related validation across `#[recallable_model]`, `#[derive(Recallable)]`, `determine_field_behavior`, and `parse_recallable_serde_attrs`. This makes it unclear which macro owns which check, leads to unreachable code paths (e.g. no-serde rejection in `parse_recallable_serde_attrs` that's never called without serde), and reports only the first error per compilation.

---

## Design Decisions

- `#[recallable_model]` is a thin orchestrator: insert derives, add `#[serde(skip/rename/alias)]`, reject manual `Serialize`. No semantic validation of rename/alias.
- `#[derive(Recallable)]` owns all serde attr validation.
- `analyze_serde_attrs` runs unconditionally (not gated behind `SERDE_ENABLED`).
- Parse failures (malformed syntax) bail immediately. Validation errors (no-serde, skip+rename, merge conflicts) are accumulated per field and reported together.
- `MergeMode` is removed — the derive always uses the same merge logic.
- The model macro's `check_no_serde_rename_or_alias` is removed — if a user writes `#[serde(rename)]` alongside `#[recallable(rename)]` under `#[recallable_model]`, the derive handles merge/conflict naturally.

---

## Changes by File

### `model_macro.rs` — strip to thin orchestrator

`expand_tokens` becomes:

```rust
fn expand_tokens(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    validate_model_attr(&attr)?;
    let mut model_item = parse_model_item_tokens(item)?;
    let derive_input = model_item.parse();
    context::analyze_model_input(&derive_input)?;
    if SERDE_ENABLED {
        check_no_serialize_derive(model_item.attrs())?;
    }

    model_item.add_derives();
    if SERDE_ENABLED {
        model_item.add_serde_skip_attrs();
        model_item.add_serde_forwarded_attrs();
    }

    Ok(model_item.item_tokenstream())
}
```

Remove:

- `check_no_serde_rename_or_alias` and `check_no_serde_rename_or_alias_on_fields`
- `check_no_recallable_serde_attrs_without_feature` method and `check_no_recallable_serde_attrs_without_feature_on_fields`

Keep:

- `add_serde_forwarded_attrs` / `add_serde_forwarded_attrs_to_fields` (uses `parse_recallable_serde_attrs`)
- `add_serde_skip_attrs`
- `check_no_serialize_derive`

### `lib.rs` — call `analyze_serde_attrs` unconditionally

Replace:

```rust
let serde_attrs = if context::SERDE_ENABLED {
    match context::analyze_serde_attrs(&input, context::MergeMode::Derive) { ... }
} else {
    context::empty_serde_attrs(&input)
};
```

With:

```rust
let serde_attrs = match context::analyze_serde_attrs(&input) {
    Ok(attrs) => attrs,
    Err(e) => return e.to_compile_error().into(),
};
```

### `serde_attrs/merge.rs` — remove `MergeMode`

Delete `MergeMode` enum. `merge_field_attrs` drops the `mode` parameter and the `MergeMode::Model` rejection branch:

```rust
pub(crate) fn merge_field_attrs(
    recallable: RawFieldSerdeAttrs,
    serde: RawFieldSerdeAttrs,
    field_span: proc_macro2::Span,
) -> syn::Result<SerdeFieldAttrs> {
    // Merge rename — conflict if both present with different values
    // Merge aliases — union and deduplicate
}
```

### `serde_attrs/parse.rs` — pure parser

Remove `!SERDE_ENABLED` checks from `parse_recallable_serde_attrs`. Remove `use crate::context::SERDE_ENABLED`.

Remove `has_serde_rename_or_alias` function and its test.

### `serde_attrs/mod.rs` — error accumulation

`analyze_struct_serde_attrs` and `analyze_enum_serde_attrs` drop the `mode` parameter and accumulate validation errors:

Per-field logic:

1. Parse `recallable` attrs → parse failure bails immediately (`?`)
2. Parse `serde` attrs → parse failure bails immediately (`?`)
3. Validation checks (accumulated via `syn::Error::combine`):
   - If `!SERDE_ENABLED` and recallable has rename or aliases → accumulate
   - If skip field and recallable has rename or aliases → accumulate
   - Merge → if conflict → accumulate
4. If no validation errors for this field, push result

At the end: if any validation errors accumulated, return them all combined.

### `context/internal/shared/fields.rs` — revert to silent consumption

`determine_field_behavior` reverts to silently consuming `rename`/`alias`:

```rust
} else if meta.path.is_ident("rename") || meta.path.is_ident("alias") {
    // Consumed by the serde_attrs analysis pass; skip the value here.
    let _value = meta.value()?;
    let _lit: syn::LitStr = _value.parse()?;
    Ok(())
```

### `context.rs` — cleanup

- Remove `empty_serde_attrs`
- Remove `MergeMode` re-export
- Remove `has_serde_rename_or_alias` re-export
- Update `analyze_serde_attrs` signature: drop `mode` parameter

---

## Test Updates

### Remove

- `model_macro.rs`: test for `check_no_serde_rename_or_alias` rejection (if exists)
- `merge.rs`: `model_rejects_serde_rename` and `model_rejects_serde_alias` tests
- `parse.rs`: `has_serde_rename_or_alias` tests
- UI tests: `model_fail_manual_serde_rename.rs/.stderr` and `model_fail_manual_serde_alias.rs/.stderr`
- `macro_expansion_failures.rs`: remove registrations for the deleted UI tests

### Update

- `merge.rs`: all `merge_field_attrs` calls drop the `mode` parameter
- `serde_attrs/mod.rs` callers: drop `mode` parameter

### Add

- Test that `analyze_serde_attrs` accumulates multiple errors (e.g. two fields both with no-serde violations → combined error contains both)
