# Serde Validation Responsibility Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move all serde attribute validation into `#[derive(Recallable)]`, make `#[recallable_model]` a thin orchestrator, and accumulate validation errors.

**Architecture:** Remove `MergeMode`, `empty_serde_attrs`, and `has_serde_rename_or_alias`. Make `parse_recallable_serde_attrs` a pure parser. Call `analyze_serde_attrs` unconditionally in `derive_recallable`. Accumulate validation errors in `analyze_struct_serde_attrs` / `analyze_enum_serde_attrs` using `syn::Error::combine`.

**Tech Stack:** Rust proc-macro, syn, quote

---

### Task 1: Remove `MergeMode` and simplify `merge_field_attrs`

**Files:**
- Modify: `recallable-macro/src/context/internal/serde_attrs/merge.rs`
- Modify: `recallable-macro/src/context/internal/serde_attrs/mod.rs:10` (re-export)

- [ ] **Step 1: Update `merge_field_attrs` — remove `mode` parameter and Model rejection**

In `recallable-macro/src/context/internal/serde_attrs/merge.rs`, replace the entire `MergeMode` enum and `merge_field_attrs` function (lines 1–70) with:

```rust
use super::parse::RawFieldSerdeAttrs;
use super::types::SerdeFieldAttrs;

/// Merge `#[recallable(...)]` and `#[serde(...)]` attrs for a single field.
/// Returns the merged `SerdeFieldAttrs` or a compile error on conflict.
pub(crate) fn merge_field_attrs(
    recallable: RawFieldSerdeAttrs,
    serde: RawFieldSerdeAttrs,
    field_span: proc_macro2::Span,
) -> syn::Result<SerdeFieldAttrs> {
    // Merge rename
    let rename = match (recallable.rename, serde.rename) {
        (Some(r), Some(s)) => {
            if r.value() != s.value() {
                return Err(syn::Error::new(
                    field_span,
                    format!(
                        "conflicting `rename` values: `#[serde(rename = \"{}\")]` and \
                         `#[recallable(rename = \"{}\")]` must match",
                        s.value(),
                        r.value(),
                    ),
                ));
            }
            Some(r)
        }
        (Some(r), None) => Some(r),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    };

    // Merge aliases: union and deduplicate by string value
    let mut seen = std::collections::BTreeSet::new();
    let mut aliases = Vec::new();
    for lit in recallable.aliases.into_iter().chain(serde.aliases) {
        if seen.insert(lit.value()) {
            aliases.push(lit);
        }
    }

    Ok(SerdeFieldAttrs { rename, aliases })
}
```

- [ ] **Step 2: Remove model-mode tests, update remaining tests to drop `mode` parameter**

In the `#[cfg(test)] mod tests` block of the same file, remove `serde_rename_rejected_in_model_mode` and `serde_alias_rejected_in_model_mode` tests entirely.

For all remaining tests, remove the `MergeMode::Derive` argument from `merge_field_attrs` calls. For example, `both_empty_produces_empty` becomes:

```rust
    #[test]
    fn both_empty_produces_empty() {
        let result = merge_field_attrs(
            RawFieldSerdeAttrs::default(),
            RawFieldSerdeAttrs::default(),
            Span::call_site(),
        )
        .unwrap();
        assert!(result.rename.is_none() && result.aliases.is_empty());
    }
```

Apply the same change to: `recallable_rename_only`, `serde_rename_only_in_derive_mode`, `matching_rename_values_merge`, `conflicting_rename_values_rejected`, `aliases_are_unioned_and_deduplicated`.

- [ ] **Step 3: Remove `MergeMode` re-export from `serde_attrs/mod.rs`**

In `recallable-macro/src/context/internal/serde_attrs/mod.rs`, remove line 10:

```rust
pub(crate) use merge::MergeMode;
```

And update the `merge_field_attrs` call sites in this file to drop the `mode` parameter (lines 36–43 and 70–77). Change:

```rust
        let merged = merge_field_attrs(
            recallable,
            serde,
            mode,
            field.ident.as_ref()
                .map(|i| i.span())
                .unwrap_or_else(|| field.ty.span()),
        )?;
```

To:

```rust
        let merged = merge_field_attrs(
            recallable,
            serde,
            field.ident.as_ref()
                .map(|i| i.span())
                .unwrap_or_else(|| field.ty.span()),
        )?;
```

In both `analyze_struct_serde_attrs` and `analyze_enum_serde_attrs`.

- [ ] **Step 4: Run tests**

Run: `cargo test --package recallable-macro --features serde -- merge 2>&1`
Expected: All remaining merge tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove MergeMode and simplify merge_field_attrs"
```

---

### Task 2: Make `parse_recallable_serde_attrs` a pure parser

**Files:**
- Modify: `recallable-macro/src/context/internal/serde_attrs/parse.rs`

- [ ] **Step 1: Remove `!SERDE_ENABLED` checks and import**

In `recallable-macro/src/context/internal/serde_attrs/parse.rs`, remove line 3:

```rust
use crate::context::SERDE_ENABLED;
```

In `parse_recallable_serde_attrs`, remove the two `if !SERDE_ENABLED` blocks (lines 25–30 and 39–43). The `rename` branch becomes:

```rust
                if meta.path.is_ident("rename") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    if result.rename.is_some() {
                        return Err(meta.error("duplicate `rename` in `#[recallable(...)]`"));
                    }
                    result.rename = Some(lit);
                    Ok(())
```

The `alias` branch becomes:

```rust
                } else if meta.path.is_ident("alias") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    result.aliases.push(lit);
                    Ok(())
```

- [ ] **Step 2: Remove `has_serde_rename_or_alias` function**

In the same file, remove the `has_serde_rename_or_alias` function (lines 86–91):

```rust
/// Returns `true` if the field has `#[serde(rename = ...)]` or `#[serde(alias = ...)]`.
pub(crate) fn has_serde_rename_or_alias(field: &Field) -> bool {
    parse_serde_attrs(field)
        .map(|attrs| attrs.rename.is_some() || !attrs.aliases.is_empty())
        .unwrap_or(false)
}
```

- [ ] **Step 3: Remove `#[cfg(feature = "serde")]` gates from parse tests**

The tests `recallable_rename_parsed`, `recallable_alias_parsed`, and `recallable_rename_and_alias_combined` currently have `#[cfg(feature = "serde")]` because parsing used to reject without serde. Remove those `#[cfg(feature = "serde")]` attributes since parsing is now unconditional.

- [ ] **Step 4: Run tests**

Run: `cargo test --package recallable-macro --features serde -- parse 2>&1 && cargo test --package recallable-macro -- parse 2>&1`
Expected: All parse tests pass in both feature configurations.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: make parse_recallable_serde_attrs a pure parser"
```

---

### Task 3: Add error accumulation to `analyze_struct_serde_attrs` and `analyze_enum_serde_attrs`

**Files:**
- Modify: `recallable-macro/src/context/internal/serde_attrs/mod.rs`

- [ ] **Step 1: Add `SERDE_ENABLED` import**

Add to the imports at the top of `recallable-macro/src/context/internal/serde_attrs/mod.rs`:

```rust
use crate::context::SERDE_ENABLED;
```

- [ ] **Step 2: Rewrite `analyze_struct_serde_attrs` with error accumulation**

Drop the `mode` parameter. Replace the function body (the signature and `mode` parameter were already updated in Task 1 Step 3 — now also drop the `mode` from the signature):

```rust
/// Run the serde attribute analysis pass over a struct's fields.
pub(crate) fn analyze_struct_serde_attrs(
    fields: &Fields,
) -> syn::Result<SerdeStructAttrs> {
    let mut result = Vec::with_capacity(fields.len());
    let mut errors: Option<syn::Error> = None;

    for field in fields.iter() {
        let recallable = parse_recallable_serde_attrs(field)?;
        let serde = parse_serde_attrs(field)?;

        let field_span = field.ident.as_ref()
            .map(|i| i.span())
            .unwrap_or_else(|| field.ty.span());

        let mut field_ok = true;

        // Reject rename/alias without serde feature
        if !SERDE_ENABLED
            && (recallable.rename.is_some() || !recallable.aliases.is_empty())
        {
            let err = syn::Error::new(
                field_span,
                "`#[recallable(rename = \"...\")]` and `#[recallable(alias = \"...\")]` \
                 require the `serde` feature",
            );
            match &mut errors {
                Some(e) => e.combine(err),
                None => errors = Some(err),
            }
            field_ok = false;
        }

        // Reject rename/alias on skipped fields
        if has_recallable_skip_attr(field)
            && (recallable.rename.is_some() || !recallable.aliases.is_empty())
        {
            let err = syn::Error::new_spanned(
                field,
                "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
            );
            match &mut errors {
                Some(e) => e.combine(err),
                None => errors = Some(err),
            }
            field_ok = false;
        }

        // Merge
        if field_ok {
            match merge_field_attrs(recallable, serde, field_span) {
                Ok(merged) => result.push(merged),
                Err(err) => {
                    match &mut errors {
                        Some(e) => e.combine(err),
                        None => errors = Some(err),
                    }
                    result.push(SerdeFieldAttrs::default());
                }
            }
        } else {
            result.push(SerdeFieldAttrs::default());
        }
    }

    if let Some(e) = errors {
        Err(e)
    } else {
        Ok(SerdeStructAttrs { fields: result })
    }
}
```

- [ ] **Step 3: Rewrite `analyze_enum_serde_attrs` with error accumulation**

Same pattern, drop `mode` parameter:

```rust
/// Run the serde attribute analysis pass over an enum's variants.
pub(crate) fn analyze_enum_serde_attrs(
    data: &syn::DataEnum,
) -> syn::Result<SerdeEnumAttrs> {
    let mut variants = Vec::with_capacity(data.variants.len());
    let mut errors: Option<syn::Error> = None;

    for variant in &data.variants {
        let mut fields = Vec::with_capacity(variant.fields.len());
        for field in variant.fields.iter() {
            let recallable = parse_recallable_serde_attrs(field)?;
            let serde = parse_serde_attrs(field)?;

            let field_span = field.ident.as_ref()
                .map(|i| i.span())
                .unwrap_or_else(|| field.ty.span());

            let mut field_ok = true;

            if !SERDE_ENABLED
                && (recallable.rename.is_some() || !recallable.aliases.is_empty())
            {
                let err = syn::Error::new(
                    field_span,
                    "`#[recallable(rename = \"...\")]` and `#[recallable(alias = \"...\")]` \
                     require the `serde` feature",
                );
                match &mut errors {
                    Some(e) => e.combine(err),
                    None => errors = Some(err),
                }
                field_ok = false;
            }

            if has_recallable_skip_attr(field)
                && (recallable.rename.is_some() || !recallable.aliases.is_empty())
            {
                let err = syn::Error::new_spanned(
                    field,
                    "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
                );
                match &mut errors {
                    Some(e) => e.combine(err),
                    None => errors = Some(err),
                }
                field_ok = false;
            }

            if field_ok {
                match merge_field_attrs(recallable, serde, field_span) {
                    Ok(merged) => fields.push(merged),
                    Err(err) => {
                        match &mut errors {
                            Some(e) => e.combine(err),
                            None => errors = Some(err),
                        }
                        fields.push(SerdeFieldAttrs::default());
                    }
                }
            } else {
                fields.push(SerdeFieldAttrs::default());
            }
        }
        variants.push(fields);
    }

    if let Some(e) = errors {
        Err(e)
    } else {
        Ok(SerdeEnumAttrs { variants })
    }
}
```

- [ ] **Step 4: Verify `SerdeFieldAttrs` has `Default`**

Check that `SerdeFieldAttrs` derives or implements `Default`. If not, add `#[derive(Default)]` to it in `recallable-macro/src/context/internal/serde_attrs/types.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo test --package recallable-macro --features serde 2>&1 && cargo test --package recallable-macro 2>&1`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: accumulate serde validation errors in analyze_serde_attrs"
```

---

### Task 4: Call `analyze_serde_attrs` unconditionally and remove `empty_serde_attrs`

**Files:**
- Modify: `recallable-macro/src/lib.rs:109-116`
- Modify: `recallable-macro/src/context.rs`

- [ ] **Step 1: Update `derive_recallable` in `lib.rs`**

Replace lines 109–116:

```rust
    let serde_attrs = if context::SERDE_ENABLED {
        match context::analyze_serde_attrs(&input, context::MergeMode::Derive) {
            Ok(attrs) => attrs,
            Err(e) => return e.to_compile_error().into(),
        }
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

- [ ] **Step 2: Update `analyze_serde_attrs` signature in `context.rs`**

In `recallable-macro/src/context.rs`, update the function signature and body — drop `mode` parameter:

```rust
pub(super) fn analyze_serde_attrs(
    input: &DeriveInput,
) -> syn::Result<internal::serde_attrs::types::SerdeItemAttrs> {
    use internal::serde_attrs::types::SerdeItemAttrs;
    match &input.data {
        syn::Data::Struct(data) => {
            let attrs = internal::serde_attrs::analyze_struct_serde_attrs(&data.fields)?;
            Ok(SerdeItemAttrs::Struct(attrs))
        }
        syn::Data::Enum(data) => {
            let attrs = internal::serde_attrs::analyze_enum_serde_attrs(data)?;
            Ok(SerdeItemAttrs::Enum(attrs))
        }
        _ => unreachable!("unions rejected earlier"),
    }
}
```

- [ ] **Step 3: Remove `empty_serde_attrs` from `context.rs`**

Delete the `empty_serde_attrs` function (lines 82–97):

```rust
pub(super) fn empty_serde_attrs(
    input: &DeriveInput,
) -> internal::serde_attrs::types::SerdeItemAttrs {
    ...
}
```

- [ ] **Step 4: Remove dead re-exports from `context.rs`**

Remove from the re-exports:
- `MergeMode` (line 24: `pub(super) use internal::serde_attrs::MergeMode;`)
- `has_serde_rename_or_alias` (line 25: `pub(super) use internal::serde_attrs::parse::has_serde_rename_or_alias;` — check exact line)

Keep: `parse_recallable_serde_attrs` re-export (used by model_macro).

- [ ] **Step 5: Run tests**

Run: `cargo test --package recallable-macro --features serde 2>&1 && cargo test --package recallable-macro 2>&1`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: call analyze_serde_attrs unconditionally, remove empty_serde_attrs"
```

---

### Task 5: Strip `model_macro.rs` to thin orchestrator

**Files:**
- Modify: `recallable-macro/src/model_macro.rs`

- [ ] **Step 1: Simplify `expand_tokens`**

Replace the current `expand_tokens` function with:

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

- [ ] **Step 2: Remove `check_no_serde_rename_or_alias_on_fields` function**

Delete the function (around lines 84–95 in the current file):

```rust
fn check_no_serde_rename_or_alias_on_fields(fields: &Fields) -> syn::Result<()> {
    for field in fields.iter() {
        if context::has_serde_rename_or_alias(field) {
            ...
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Remove `check_no_recallable_serde_attrs_without_feature_on_fields` function**

Delete the function that was added earlier in this branch.

- [ ] **Step 4: Remove `check_no_serde_rename_or_alias` and `check_no_recallable_serde_attrs_without_feature` methods from `ModelItem`**

Remove both methods from the `impl ModelItem` block.

- [ ] **Step 5: Clean up imports**

Update line 7 from:

```rust
use crate::context::{self, SERDE_ENABLED, crate_path, has_recallable_skip_attr};
```

To (remove `has_serde_rename_or_alias` if present — verify it's not there, it's used qualified). The import should remain:

```rust
use crate::context::{self, SERDE_ENABLED, crate_path, has_recallable_skip_attr};
```

No change needed if `has_serde_rename_or_alias` was used qualified via `context::has_serde_rename_or_alias`. But since the call site is removed, the import `context` still covers `context::parse_recallable_serde_attrs` and `context::analyze_model_input`.

- [ ] **Step 6: Run tests**

Run: `cargo test --package recallable-macro --features serde 2>&1 && cargo test --package recallable-macro 2>&1`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: strip model_macro to thin orchestrator"
```

---

### Task 6: Revert `determine_field_behavior` to silent consumption

**Files:**
- Modify: `recallable-macro/src/context/internal/shared/fields.rs:62-74`

- [ ] **Step 1: Verify current state**

Read `recallable-macro/src/context/internal/shared/fields.rs` lines 57–75. The `rename`/`alias` branch should already be silently consuming (based on the user's earlier edit). Confirm it looks like:

```rust
            } else if meta.path.is_ident("rename") || meta.path.is_ident("alias") {
                // Consumed by the serde_attrs analysis pass; skip the value here.
                let _value = meta.value()?;
                let _lit: syn::LitStr = _value.parse()?;
                Ok(())
```

If it has any `!SERDE_ENABLED` check, remove it.

- [ ] **Step 2: Run tests**

Run: `cargo test --package recallable-macro --features serde 2>&1 && cargo test --package recallable-macro 2>&1`
Expected: All tests pass.

- [ ] **Step 3: Commit (if changes were needed)**

```bash
git add -A
git commit -m "refactor: revert determine_field_behavior to silent rename/alias consumption"
```

---

### Task 7: Remove UI tests for model serde rename/alias rejection

**Files:**
- Delete: `recallable/tests/ui/model_fail_manual_serde_rename.rs`
- Delete: `recallable/tests/ui/model_fail_manual_serde_rename.stderr`
- Delete: `recallable/tests/ui/model_fail_manual_serde_alias.rs`
- Delete: `recallable/tests/ui/model_fail_manual_serde_alias.stderr`
- Modify: `recallable/tests/macro_expansion_failures.rs:29-30`

- [ ] **Step 1: Delete the UI test files**

```bash
rm recallable/tests/ui/model_fail_manual_serde_rename.rs
rm recallable/tests/ui/model_fail_manual_serde_rename.stderr
rm recallable/tests/ui/model_fail_manual_serde_alias.rs
rm recallable/tests/ui/model_fail_manual_serde_alias.stderr
```

- [ ] **Step 2: Remove registrations from `macro_expansion_failures.rs`**

In `recallable/tests/macro_expansion_failures.rs`, remove lines 29–30:

```rust
        tests.compile_fail("tests/ui/model_fail_manual_serde_rename.rs");
        tests.compile_fail("tests/ui/model_fail_manual_serde_alias.rs");
```

- [ ] **Step 3: Run UI tests**

Run: `cargo test --package recallable --features serde -- macro_expansion_failures 2>&1`
Expected: All remaining UI tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test: remove model serde rename/alias rejection UI tests"
```

---

### Task 8: Add error accumulation test

**Files:**
- Modify: `recallable-macro/src/context/internal/serde_attrs/mod.rs` (add test module)

- [ ] **Step 1: Write the test**

Add a `#[cfg(test)]` module at the bottom of `recallable-macro/src/context/internal/serde_attrs/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use syn::{parse_quote, Fields};

    fn struct_fields(input: &syn::ItemStruct) -> &Fields {
        &input.fields
    }

    #[cfg(not(feature = "serde"))]
    #[test]
    fn accumulates_no_serde_errors_across_fields() {
        let input: syn::ItemStruct = parse_quote! {
            struct Example {
                #[recallable(rename = "a")]
                first: i32,
                #[recallable(alias = "b")]
                second: i32,
            }
        };
        let err = analyze_struct_serde_attrs(struct_fields(&input)).unwrap_err();
        let msg = err.to_string();
        // Both fields should be reported
        assert!(msg.contains("serde"), "expected serde feature error, got: {msg}");
        // syn::Error with combine produces multiple error messages joined
        let errors: Vec<_> = err.into_iter().collect();
        assert_eq!(errors.len(), 2, "expected 2 accumulated errors, got {}", errors.len());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn accumulates_skip_rename_errors_across_fields() {
        let input: syn::ItemStruct = parse_quote! {
            struct Example {
                #[recallable(skip, rename = "a")]
                first: i32,
                #[recallable(skip, alias = "b")]
                second: i32,
            }
        };
        let err = analyze_struct_serde_attrs(struct_fields(&input)).unwrap_err();
        let errors: Vec<_> = err.into_iter().collect();
        assert_eq!(errors.len(), 2, "expected 2 accumulated errors, got {}", errors.len());
    }
}
```

- [ ] **Step 2: Run the test (no-serde config)**

Run: `cargo test --package recallable-macro -- accumulates 2>&1`
Expected: `accumulates_no_serde_errors_across_fields` passes.

- [ ] **Step 3: Run the test (serde config)**

Run: `cargo test --package recallable-macro --features serde -- accumulates 2>&1`
Expected: `accumulates_skip_rename_errors_across_fields` passes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test: add error accumulation tests for analyze_serde_attrs"
```

---

### Task 9: Full test suite and clippy

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace --features serde 2>&1 && cargo test --workspace 2>&1`
Expected: All tests pass in both feature configurations.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --features serde -- -D warnings 2>&1 && cargo clippy --workspace -- -D warnings 2>&1`
Expected: No warnings.

- [ ] **Step 3: Commit (if any fixups needed)**

```bash
git add -A
git commit -m "chore: fixups from full test suite"
```
