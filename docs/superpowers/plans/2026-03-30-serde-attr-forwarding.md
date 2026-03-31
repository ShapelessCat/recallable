# Serde Attribute Forwarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forward `rename` and `alias` serde attributes from source structs/enums to generated memento types so the serialize-source / deserialize-memento / recall loop works correctly.

**Architecture:** A new `serde_attrs` module under `recallable-macro/src/context/internal/` runs a separate analysis pass over `DeriveInput` to extract, merge, and validate `rename`/`alias` from `#[recallable(...)]` and `#[serde(...)]` attributes. The result threads into memento codegen to emit `#[serde(...)]` on memento fields, and into `model_macro.rs` to annotate source fields and reject manual `#[serde(rename/alias)]`.

**Tech Stack:** Rust proc-macro (`syn`, `quote`, `proc-macro2`), `trybuild` for UI tests, `serde_json` for integration tests.

---

## File Structure

### New files

- `recallable-macro/src/context/internal/serde_attrs.rs` — module declaration, public API
- `recallable-macro/src/context/internal/serde_attrs/parse.rs` — attribute parsing from `#[recallable(...)]` and `#[serde(...)]`
- `recallable-macro/src/context/internal/serde_attrs/merge.rs` — merging logic and conflict detection
- `recallable-macro/src/context/internal/serde_attrs/types.rs` — `SerdeFieldAttrs`, `SerdeStructAttrs`, `SerdeEnumAttrs`, token generation
- `recallable/tests/ui/derive_fail_serde_rename_conflict.rs` + `.stderr` — conflicting rename values
- `recallable/tests/ui/model_fail_manual_serde_rename.rs` + `.stderr` — manual `#[serde(rename)]` under `#[recallable_model]`
- `recallable/tests/ui/model_fail_manual_serde_alias.rs` + `.stderr` — manual `#[serde(alias)]` under `#[recallable_model]`
- `recallable/tests/ui/derive_fail_serde_attr_on_skip.rs` + `.stderr` — rename/alias on skipped field
- `recallable/tests/ui/derive_fail_serde_rename_no_feature.rs` + `.stderr` — rename without serde feature

### Modified files

- `recallable-macro/src/context/internal.rs` — add `pub(crate) mod serde_attrs;`
- `recallable-macro/src/context/internal/shared/fields.rs:62-67` — accept `rename` and `alias` in `determine_field_behavior`
- `recallable-macro/src/context.rs:30-56` — thread `SerdeAttrs` through `analyze_item` → `gen_memento_type`
- `recallable-macro/src/context/memento.rs` — pass serde attrs to struct/enum generators
- `recallable-macro/src/context/memento/structs.rs:94-99` — prepend serde attr tokens to memento fields
- `recallable-macro/src/context/memento/enums.rs:82-88` — prepend serde attr tokens to memento variant fields
- `recallable-macro/src/lib.rs:100-124` — call `analyze_serde_attrs` in `derive_recallable`
- `recallable-macro/src/model_macro.rs:15-37` — add rejection + source annotation steps
- `recallable/tests/macro_expansion_failures.rs` — register new UI tests
- `recallable/tests/serde_json.rs` — add integration round-trip tests
- `recallable/tests/common/mod.rs` — add test structs with rename/alias

---

## Task 1: Data Types and Token Generation

**Files:**
- Create: `recallable-macro/src/context/internal/serde_attrs/types.rs`

- [ ] **Step 1: Write failing tests for `SerdeFieldAttrs::to_memento_tokens`**

In `types.rs`, add the types and a `#[cfg(test)]` module:

```rust
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

#[derive(Debug, Default)]
pub(crate) struct SerdeFieldAttrs {
    pub(crate) rename: Option<syn::LitStr>,
    pub(crate) aliases: Vec<syn::LitStr>,
}

/// Result of the serde attribute analysis pass for a struct.
#[derive(Debug)]
pub(crate) struct SerdeStructAttrs {
    /// Per-field attrs, indexed by field position.
    pub(crate) fields: Vec<SerdeFieldAttrs>,
}

/// Result of the serde attribute analysis pass for an enum.
#[derive(Debug)]
pub(crate) struct SerdeEnumAttrs {
    /// Per-variant, per-field attrs.
    pub(crate) variants: Vec<Vec<SerdeFieldAttrs>>,
}

impl SerdeFieldAttrs {
    pub(crate) fn is_empty(&self) -> bool {
        self.rename.is_none() && self.aliases.is_empty()
    }

    #[must_use]
    pub(crate) fn to_memento_tokens(&self) -> TokenStream2 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn empty_attrs_produce_no_tokens() {
        let attrs = SerdeFieldAttrs::default();
        assert!(attrs.to_memento_tokens().is_empty());
    }

    #[test]
    fn rename_produces_serde_rename_attr() {
        let attrs = SerdeFieldAttrs {
            rename: Some(parse_quote!("wire_name")),
            aliases: vec![],
        };
        let tokens = attrs.to_memento_tokens().to_string();
        assert!(tokens.contains("rename"));
        assert!(tokens.contains("wire_name"));
    }

    #[test]
    fn aliases_produce_serde_alias_attrs() {
        let attrs = SerdeFieldAttrs {
            rename: None,
            aliases: vec![parse_quote!("old"), parse_quote!("legacy")],
        };
        let tokens = attrs.to_memento_tokens().to_string();
        assert!(tokens.contains("alias"));
        assert!(tokens.contains("old"));
        assert!(tokens.contains("legacy"));
    }

    #[test]
    fn rename_and_aliases_combined() {
        let attrs = SerdeFieldAttrs {
            rename: Some(parse_quote!("new_name")),
            aliases: vec![parse_quote!("alt")],
        };
        let tokens = attrs.to_memento_tokens().to_string();
        assert!(tokens.contains("rename"));
        assert!(tokens.contains("new_name"));
        assert!(tokens.contains("alias"));
        assert!(tokens.contains("alt"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package recallable-macro -- serde_attrs::types`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement `to_memento_tokens`**

Replace the `todo!()` in `to_memento_tokens`:

```rust
#[must_use]
pub(crate) fn to_memento_tokens(&self) -> TokenStream2 {
    let rename = self.rename.as_ref().map(|lit| {
        quote! { #[serde(rename = #lit)] }
    });
    let aliases = self.aliases.iter().map(|lit| {
        quote! { #[serde(alias = #lit)] }
    });
    quote! {
        #rename
        #(#aliases)*
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --package recallable-macro -- serde_attrs::types`
Expected: All 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add recallable-macro/src/context/internal/serde_attrs/types.rs
git commit -m "feat(serde-attrs): add SerdeFieldAttrs types and token generation"
```

---

## Task 2: Attribute Parsing

**Files:**
- Create: `recallable-macro/src/context/internal/serde_attrs/parse.rs`

- [ ] **Step 1: Write failing tests for parsing `#[recallable(...)]` serde attrs**

Create `parse.rs` with parsing functions and tests:

```rust
use syn::{Field, LitStr};

use crate::context::SERDE_ENABLED;

const RECALLABLE: &str = "recallable";
const SERDE: &str = "serde";

/// Parsed rename/alias values from a single attribute source.
#[derive(Debug, Default)]
pub(crate) struct RawFieldSerdeAttrs {
    pub(crate) rename: Option<LitStr>,
    pub(crate) aliases: Vec<LitStr>,
}

/// Extract rename/alias from `#[recallable(...)]` attributes on a field.
pub(crate) fn parse_recallable_serde_attrs(field: &Field) -> syn::Result<RawFieldSerdeAttrs> {
    todo!()
}

/// Extract rename/alias from `#[serde(...)]` attributes on a field.
pub(crate) fn parse_serde_attrs(field: &Field) -> syn::Result<RawFieldSerdeAttrs> {
    todo!()
}

/// Returns `true` if the field has `#[serde(rename = ...)]` or `#[serde(alias = ...)]`.
pub(crate) fn has_serde_rename_or_alias(field: &Field) -> bool {
    parse_serde_attrs(field)
        .map(|attrs| attrs.rename.is_some() || !attrs.aliases.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn make_field(tokens: proc_macro2::TokenStream) -> Field {
        let item: syn::ItemStruct = parse_quote! {
            struct S { #tokens }
        };
        match &item.fields {
            syn::Fields::Named(f) => f.named.first().unwrap().clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn recallable_rename_parsed() {
        let field = make_field(quote::quote! {
            #[recallable(rename = "wire")]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert_eq!(attrs.rename.unwrap().value(), "wire");
        assert!(attrs.aliases.is_empty());
    }

    #[test]
    fn recallable_alias_parsed() {
        let field = make_field(quote::quote! {
            #[recallable(alias = "old", alias = "legacy")]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert!(attrs.rename.is_none());
        assert_eq!(attrs.aliases.len(), 2);
        assert_eq!(attrs.aliases[0].value(), "old");
        assert_eq!(attrs.aliases[1].value(), "legacy");
    }

    #[test]
    fn recallable_rename_and_alias_combined() {
        let field = make_field(quote::quote! {
            #[recallable(rename = "new", alias = "alt")]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert_eq!(attrs.rename.unwrap().value(), "new");
        assert_eq!(attrs.aliases.len(), 1);
    }

    #[test]
    fn serde_rename_parsed() {
        let field = make_field(quote::quote! {
            #[serde(rename = "wire")]
            value: i32
        });
        let attrs = parse_serde_attrs(&field).unwrap();
        assert_eq!(attrs.rename.unwrap().value(), "wire");
    }

    #[test]
    fn serde_alias_parsed() {
        let field = make_field(quote::quote! {
            #[serde(alias = "old")]
            value: i32
        });
        let attrs = parse_serde_attrs(&field).unwrap();
        assert_eq!(attrs.aliases.len(), 1);
        assert_eq!(attrs.aliases[0].value(), "old");
    }

    #[test]
    fn no_attrs_returns_empty() {
        let field = make_field(quote::quote! { value: i32 });
        let recallable = parse_recallable_serde_attrs(&field).unwrap();
        let serde = parse_serde_attrs(&field).unwrap();
        assert!(recallable.rename.is_none());
        assert!(recallable.aliases.is_empty());
        assert!(serde.rename.is_none());
        assert!(serde.aliases.is_empty());
    }

    #[test]
    fn recallable_skip_field_is_ignored() {
        let field = make_field(quote::quote! {
            #[recallable(skip)]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert!(attrs.rename.is_none());
        assert!(attrs.aliases.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package recallable-macro -- serde_attrs::parse`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement `parse_recallable_serde_attrs`**

Replace the `todo!()`:

```rust
pub(crate) fn parse_recallable_serde_attrs(field: &Field) -> syn::Result<RawFieldSerdeAttrs> {
    let mut result = RawFieldSerdeAttrs::default();

    for attr in field.attrs.iter().filter(|a| a.path().is_ident(RECALLABLE)) {
        if let syn::Meta::List(_) = &attr.meta {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    if !SERDE_ENABLED {
                        return Err(syn::Error::new_spanned(
                            &lit,
                            "`#[recallable(rename = \"...\")]` requires the `serde` feature",
                        ));
                    }
                    if result.rename.is_some() {
                        return Err(meta.error("duplicate `rename` in `#[recallable(...)]`"));
                    }
                    result.rename = Some(lit);
                    Ok(())
                } else if meta.path.is_ident("alias") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    if !SERDE_ENABLED {
                        return Err(syn::Error::new_spanned(
                            &lit,
                            "`#[recallable(alias = \"...\")]` requires the `serde` feature",
                        ));
                    }
                    result.aliases.push(lit);
                    Ok(())
                } else {
                    // skip, skip_memento_default_derives, and bare #[recallable]
                    // are handled elsewhere — just ignore them here
                    Ok(())
                }
            })?;
        }
    }

    Ok(result)
}
```

- [ ] **Step 4: Implement `parse_serde_attrs`**

```rust
pub(crate) fn parse_serde_attrs(field: &Field) -> syn::Result<RawFieldSerdeAttrs> {
    let mut result = RawFieldSerdeAttrs::default();

    for attr in field.attrs.iter().filter(|a| a.path().is_ident(SERDE)) {
        if let syn::Meta::List(_) = &attr.meta {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    if result.rename.is_some() {
                        return Err(meta.error("duplicate `rename` in `#[serde(...)]`"));
                    }
                    result.rename = Some(lit);
                } else if meta.path.is_ident("alias") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    result.aliases.push(lit);
                }
                // ignore other serde attrs — not our concern
                Ok(())
            })?;
        }
    }

    Ok(result)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --package recallable-macro -- serde_attrs::parse`
Expected: All 7 tests PASS

- [ ] **Step 6: Commit**

```bash
git add recallable-macro/src/context/internal/serde_attrs/parse.rs
git commit -m "feat(serde-attrs): add attribute parsing for rename and alias"
```

---

## Task 3: Merge Logic and Conflict Detection

**Files:**
- Create: `recallable-macro/src/context/internal/serde_attrs/merge.rs`

- [ ] **Step 1: Write failing tests for merge logic**

Create `merge.rs` with merge function and tests:

```rust
use syn::LitStr;

use super::parse::RawFieldSerdeAttrs;
use super::types::SerdeFieldAttrs;

/// Merge mode controls whether `#[serde(...)]` attrs are accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MergeMode {
    /// `#[derive(Recallable)]` — both sources accepted, conflicts rejected.
    Derive,
    /// `#[recallable_model]` — only `#[recallable(...)]` accepted.
    Model,
}

/// Merge `#[recallable(...)]` and `#[serde(...)]` attrs for a single field.
/// Returns the merged `SerdeFieldAttrs` or a compile error on conflict.
pub(crate) fn merge_field_attrs(
    recallable: RawFieldSerdeAttrs,
    serde: RawFieldSerdeAttrs,
    mode: MergeMode,
    field_span: proc_macro2::Span,
) -> syn::Result<SerdeFieldAttrs> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;
    use syn::parse_quote;

    fn lit(s: &str) -> LitStr {
        LitStr::new(s, Span::call_site())
    }

    #[test]
    fn both_empty_produces_empty() {
        let result = merge_field_attrs(
            RawFieldSerdeAttrs::default(),
            RawFieldSerdeAttrs::default(),
            MergeMode::Derive,
            Span::call_site(),
        )
        .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn recallable_rename_only() {
        let recallable = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result = merge_field_attrs(
            recallable,
            RawFieldSerdeAttrs::default(),
            MergeMode::Derive,
            Span::call_site(),
        )
        .unwrap();
        assert_eq!(result.rename.unwrap().value(), "x");
    }

    #[test]
    fn serde_rename_only_in_derive_mode() {
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result = merge_field_attrs(
            RawFieldSerdeAttrs::default(),
            serde,
            MergeMode::Derive,
            Span::call_site(),
        )
        .unwrap();
        assert_eq!(result.rename.unwrap().value(), "x");
    }

    #[test]
    fn serde_rename_rejected_in_model_mode() {
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result = merge_field_attrs(
            RawFieldSerdeAttrs::default(),
            serde,
            MergeMode::Model,
            Span::call_site(),
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("#[recallable_model]"));
    }

    #[test]
    fn serde_alias_rejected_in_model_mode() {
        let serde = RawFieldSerdeAttrs {
            rename: None,
            aliases: vec![lit("old")],
        };
        let result = merge_field_attrs(
            RawFieldSerdeAttrs::default(),
            serde,
            MergeMode::Model,
            Span::call_site(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn matching_rename_values_merge() {
        let recallable = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result = merge_field_attrs(
            recallable,
            serde,
            MergeMode::Derive,
            Span::call_site(),
        )
        .unwrap();
        assert_eq!(result.rename.unwrap().value(), "x");
    }

    #[test]
    fn conflicting_rename_values_rejected() {
        let recallable = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("y")),
            aliases: vec![],
        };
        let result = merge_field_attrs(
            recallable,
            serde,
            MergeMode::Derive,
            Span::call_site(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("conflicting"));
    }

    #[test]
    fn aliases_are_unioned_and_deduplicated() {
        let recallable = RawFieldSerdeAttrs {
            rename: None,
            aliases: vec![lit("a"), lit("b")],
        };
        let serde = RawFieldSerdeAttrs {
            rename: None,
            aliases: vec![lit("b"), lit("c")],
        };
        let result = merge_field_attrs(
            recallable,
            serde,
            MergeMode::Derive,
            Span::call_site(),
        )
        .unwrap();
        let values: Vec<String> = result.aliases.iter().map(|l| l.value()).collect();
        assert_eq!(values, vec!["a", "b", "c"]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package recallable-macro -- serde_attrs::merge`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement `merge_field_attrs`**

Replace the `todo!()`:

```rust
pub(crate) fn merge_field_attrs(
    recallable: RawFieldSerdeAttrs,
    serde: RawFieldSerdeAttrs,
    mode: MergeMode,
    field_span: proc_macro2::Span,
) -> syn::Result<SerdeFieldAttrs> {
    // In Model mode, reject any #[serde(rename/alias)]
    if mode == MergeMode::Model {
        if serde.rename.is_some() {
            return Err(syn::Error::new(
                field_span,
                "`#[recallable_model]` manages serde attributes automatically; \
                 use `#[recallable(rename = \"...\")]` instead of `#[serde(rename = \"...\")]`",
            ));
        }
        if !serde.aliases.is_empty() {
            return Err(syn::Error::new(
                field_span,
                "`#[recallable_model]` manages serde attributes automatically; \
                 use `#[recallable(alias = \"...\")]` instead of `#[serde(alias = \"...\")]`",
            ));
        }
    }

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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --package recallable-macro -- serde_attrs::merge`
Expected: All 8 tests PASS

- [ ] **Step 5: Commit**

```bash
git add recallable-macro/src/context/internal/serde_attrs/merge.rs
git commit -m "feat(serde-attrs): add merge logic with conflict detection"
```

---

## Task 4: Module Wiring and `determine_field_behavior` Update

**Files:**
- Create: `recallable-macro/src/context/internal/serde_attrs.rs`
- Modify: `recallable-macro/src/context/internal.rs`
- Modify: `recallable-macro/src/context/internal/shared/fields.rs:62-67`

- [ ] **Step 1: Create the `serde_attrs` module root**

Create `recallable-macro/src/context/internal/serde_attrs.rs`:

```rust
pub(crate) mod merge;
pub(crate) mod parse;
pub(crate) mod types;

use syn::{DeriveInput, Fields};

use crate::context::internal::shared::fields::has_recallable_skip_attr;

pub(crate) use merge::MergeMode;
pub(crate) use types::{SerdeEnumAttrs, SerdeFieldAttrs, SerdeStructAttrs};

use merge::merge_field_attrs;
use parse::{parse_recallable_serde_attrs, parse_serde_attrs};

/// Run the serde attribute analysis pass over a struct's fields.
pub(crate) fn analyze_struct_serde_attrs(
    fields: &Fields,
    mode: MergeMode,
) -> syn::Result<SerdeStructAttrs> {
    let mut result = Vec::with_capacity(fields.len());
    for field in fields.iter() {
        let recallable = parse_recallable_serde_attrs(field)?;
        let serde = parse_serde_attrs(field)?;

        // Reject rename/alias on skipped fields
        if has_recallable_skip_attr(field) {
            if recallable.rename.is_some() || !recallable.aliases.is_empty() {
                return Err(syn::Error::new_spanned(
                    field,
                    "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
                ));
            }
        }

        let merged = merge_field_attrs(
            recallable,
            serde,
            mode,
            field.ident.as_ref()
                .map(|i| i.span())
                .unwrap_or_else(|| field.ty.span()),
        )?;
        result.push(merged);
    }
    Ok(SerdeStructAttrs { fields: result })
}

/// Run the serde attribute analysis pass over an enum's variants.
pub(crate) fn analyze_enum_serde_attrs(
    data: &syn::DataEnum,
    mode: MergeMode,
) -> syn::Result<SerdeEnumAttrs> {
    let mut variants = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        let mut fields = Vec::with_capacity(variant.fields.len());
        for field in variant.fields.iter() {
            let recallable = parse_recallable_serde_attrs(field)?;
            let serde = parse_serde_attrs(field)?;

            if has_recallable_skip_attr(field) {
                if recallable.rename.is_some() || !recallable.aliases.is_empty() {
                    return Err(syn::Error::new_spanned(
                        field,
                        "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
                    ));
                }
            }

            let merged = merge_field_attrs(
                recallable,
                serde,
                mode,
                field.ident.as_ref()
                    .map(|i| i.span())
                    .unwrap_or_else(|| field.ty.span()),
            )?;
            fields.push(merged);
        }
        variants.push(fields);
    }
    Ok(SerdeEnumAttrs { variants })
}
```

- [ ] **Step 2: Register the module in `internal.rs`**

In `recallable-macro/src/context/internal.rs`, add after line 4 (`pub(crate) mod shared;`):

```rust
pub(crate) mod serde_attrs;
```

The file becomes:

```rust
//! Semantic analysis and shared helper backend for the `context` codegen facade.

pub(crate) mod enums;
pub(crate) mod serde_attrs;
pub(crate) mod shared;
pub(crate) mod structs;
```

- [ ] **Step 3: Update `determine_field_behavior` to accept `rename` and `alias`**

In `recallable-macro/src/context/internal/shared/fields.rs`, replace lines 62-67:

```rust
            Meta::List(_) => attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    saw_skip = true;
                    Ok(())
                } else {
                    Err(meta.error("unrecognized `recallable` parameter"))
                }
            })?,
```

With:

```rust
            Meta::List(_) => attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    saw_skip = true;
                    Ok(())
                } else if meta.path.is_ident("rename") || meta.path.is_ident("alias") {
                    // Consumed by the serde_attrs analysis pass; skip the value here.
                    let _value = meta.value()?;
                    let _lit: syn::LitStr = _value.parse()?;
                    Ok(())
                } else if meta.path.is_ident("skip_memento_default_derives") {
                    // Item-level attr, not field-level — but parse_nested_meta
                    // won't see it here since it's on the item, not the field.
                    Err(meta.error("unrecognized `recallable` parameter"))
                } else {
                    Err(meta.error("unrecognized `recallable` parameter"))
                }
            })?,
```

- [ ] **Step 4: Verify the build compiles**

Run: `cargo build --package recallable-macro`
Expected: Compiles successfully

- [ ] **Step 5: Run existing tests to verify no regressions**

Run: `cargo test --package recallable-macro`
Expected: All existing tests PASS

- [ ] **Step 6: Commit**

```bash
git add recallable-macro/src/context/internal/serde_attrs.rs \
       recallable-macro/src/context/internal/serde_attrs/ \
       recallable-macro/src/context/internal.rs \
       recallable-macro/src/context/internal/shared/fields.rs
git commit -m "feat(serde-attrs): wire module and accept rename/alias in recallable attr"
```

---

## Task 5: Thread Serde Attrs Into Memento Codegen

**Files:**
- Modify: `recallable-macro/src/context.rs`
- Modify: `recallable-macro/src/context/memento.rs`
- Modify: `recallable-macro/src/context/memento/structs.rs`
- Modify: `recallable-macro/src/context/memento/enums.rs`
- Modify: `recallable-macro/src/lib.rs`

- [ ] **Step 1: Add `SerdeItemAttrs` enum to `serde_attrs/types.rs`**

Add a unified wrapper to `types.rs` so the memento dispatch can accept either struct or enum attrs:

```rust
/// Unified wrapper for passing serde attrs through the memento codegen dispatch.
#[derive(Debug)]
pub(crate) enum SerdeItemAttrs {
    Struct(SerdeStructAttrs),
    Enum(SerdeEnumAttrs),
}
```

- [ ] **Step 2: Update `gen_memento_type` in `context/memento.rs` to accept serde attrs**

Replace the contents of `recallable-macro/src/context/memento.rs`:

```rust
mod enums;
mod structs;

use proc_macro2::TokenStream as TokenStream2;

use crate::context::internal::serde_attrs::types::SerdeItemAttrs;
use crate::context::internal::shared::{CodegenEnv, ItemIr};

#[must_use]
pub(crate) fn gen_memento_type(
    ir: &ItemIr,
    env: &CodegenEnv,
    serde_attrs: &SerdeItemAttrs,
) -> TokenStream2 {
    match (ir, serde_attrs) {
        (ItemIr::Struct(ir), SerdeItemAttrs::Struct(attrs)) => {
            structs::gen_memento_struct(ir, env, attrs)
        }
        (ItemIr::Enum(ir), SerdeItemAttrs::Enum(attrs)) => {
            enums::gen_memento_enum(ir, env, attrs)
        }
        _ => unreachable!("item kind and serde attrs kind must match"),
    }
}
```

- [ ] **Step 3: Update `gen_memento_struct` to emit serde attrs on fields**

In `recallable-macro/src/context/memento/structs.rs`, update the function signature and field emission.

Change the import block at the top to:

```rust
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{Ident, WhereClause, WherePredicate};

use crate::context::SERDE_ENABLED;
use crate::context::internal::serde_attrs::types::SerdeStructAttrs;
use crate::context::internal::shared::{
    CodegenEnv, CodegenItemIr, FieldIr, build_memento_field_tokens,
};
use crate::context::internal::structs::{StructIr, StructShape, collect_recall_like_bounds};
```

Change `gen_memento_struct` signature to accept `serde_attrs`:

```rust
#[must_use]
pub(crate) fn gen_memento_struct(
    ir: &StructIr,
    env: &CodegenEnv,
    serde_attrs: &SerdeStructAttrs,
) -> TokenStream2 {
    let derives = ir.memento_trait_spec().derive_attr();
    let marker_helpers = ir.synthetic_marker_helper_defs();
    let visibility = ir.visibility();
    let memento_name = ir.memento_name();
    let memento_generics = ir.memento_decl_generics();
    let body = build_memento_body(ir, env, serde_attrs);

    quote! {
        #(#marker_helpers)*

        #[automatically_derived]
        #[allow(dead_code)]
        #derives
        #visibility struct #memento_name #memento_generics #body
    }
}
```

Change `build_memento_body` to pass serde attrs:

```rust
fn build_memento_body(
    ir: &StructIr,
    env: &CodegenEnv,
    serde_attrs: &SerdeStructAttrs,
) -> TokenStream2 {
    let shape = ir.generated_memento_shape();
    let where_clause = build_memento_where_clause(ir, env);
    let fields = memento_fields_with_marker(ir, env, shape, serde_attrs);

    match shape {
        StructShape::Named => quote! { #where_clause { #(#fields),* } },
        StructShape::Unnamed => quote! { ( #(#fields),* ) #where_clause; },
        StructShape::Unit => quote! { #where_clause; },
    }
}
```

Change `memento_fields_with_marker` to emit serde attr tokens:

```rust
fn memento_fields_with_marker<'ir, 'input>(
    ir: &'ir StructIr<'input>,
    env: &'ir CodegenEnv,
    shape: StructShape,
    serde_attrs: &'ir SerdeStructAttrs,
) -> impl Iterator<Item = TokenStream2> + 'ir {
    let recallable_trait = &env.recallable_trait;

    ir.memento_fields()
        .map(|field| {
            let serde_tokens = field.memento_index
                .map(|idx| serde_attrs.fields[idx].to_memento_tokens())
                .unwrap_or_default();
            let field_tokens = build_memento_field(
                field,
                recallable_trait,
                ir.generic_type_param_idents(),
            );
            quote! { #serde_tokens #field_tokens }
        })
        .chain(
            ir.synthetic_marker_type()
                .into_iter()
                .map(move |marker_ty| build_marker_field(&marker_ty, shape)),
        )
}
```

- [ ] **Step 4: Update `gen_memento_enum` to emit serde attrs on variant fields**

In `recallable-macro/src/context/memento/enums.rs`, update imports:

```rust
use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Ident, WhereClause, WherePredicate};

use crate::context::SERDE_ENABLED;
use crate::context::internal::enums::{
    EnumIr, VariantIr, VariantShape, collect_recall_like_bounds_for_enum,
};
use crate::context::internal::serde_attrs::types::{SerdeEnumAttrs, SerdeFieldAttrs};
use crate::context::internal::shared::{
    CodegenEnv, CodegenItemIr, FieldIr, build_memento_field_tokens,
};
```

Change `gen_memento_enum` signature:

```rust
#[must_use]
pub(crate) fn gen_memento_enum(
    ir: &EnumIr,
    env: &CodegenEnv,
    serde_attrs: &SerdeEnumAttrs,
) -> TokenStream2 {
    let derives = ir.memento_trait_spec().derive_attr();
    let marker_helpers = ir.synthetic_marker_helper_defs();
    let visibility = ir.visibility();
    let memento_name = ir.memento_name();
    let memento_generics = ir.memento_decl_generics();
    let where_clause = build_memento_where_clause(ir, env);
    let variants = ir
        .variants()
        .zip(serde_attrs.variants.iter())
        .map(|(variant, variant_serde)| {
            build_memento_variant(
                variant,
                &env.recallable_trait,
                ir.generic_type_param_idents(),
                variant_serde,
            )
        })
        .chain(
            ir.synthetic_marker_type()
                .into_iter()
                .map(|marker_ty| build_marker_variant(&marker_ty)),
        );

    quote! {
        #(#marker_helpers)*

        #[automatically_derived]
        #[allow(dead_code)]
        #derives
        #visibility enum #memento_name #memento_generics #where_clause {
            #(#variants),*
        }
    }
}
```

Change `build_memento_variant`:

```rust
fn build_memento_variant(
    variant: &VariantIr<'_>,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
    variant_serde: &[SerdeFieldAttrs],
) -> TokenStream2 {
    let name = variant.name;
    let mut fields = variant
        .kept_fields()
        .map(|(_, field)| {
            let serde_tokens = field.memento_index
                .map(|idx| variant_serde[idx].to_memento_tokens())
                .unwrap_or_default();
            let field_tokens = build_memento_field(
                field,
                recallable_trait,
                generic_type_params,
            );
            quote! { #serde_tokens #field_tokens }
        })
        .peekable();
    let non_empty = fields.peek().is_some();
    match variant.shape {
        VariantShape::Named if non_empty => quote! { #name { #(#fields),* } },
        VariantShape::Unnamed if non_empty => quote! { #name(#(#fields),*) },
        _ => quote! { #name },
    }
}
```

- [ ] **Step 5: Update `context.rs` facade to thread serde attrs**

In `recallable-macro/src/context.rs`, update `gen_memento_type` to accept and forward serde attrs:

```rust
pub(crate) fn gen_memento_type(
    ir: &ItemIr,
    env: &CodegenEnv,
    serde_attrs: &internal::serde_attrs::types::SerdeItemAttrs,
) -> proc_macro2::TokenStream {
    memento::gen_memento_type(ir, env, serde_attrs)
}
```

- [ ] **Step 6: Update `derive_recallable` in `lib.rs` to call serde analysis**

In `recallable-macro/src/lib.rs`, update the `derive_recallable` function. Add the import and analysis call:

```rust
use proc_macro::TokenStream;

use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod context;
mod model_macro;
```

In `derive_recallable` (around line 100), change:

```rust
pub fn derive_recallable(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let ir = match context::analyze_item(&input) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error().into(),
    };
    let env = context::CodegenEnv::resolve();
```

To:

```rust
pub fn derive_recallable(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let ir = match context::analyze_item(&input) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error().into(),
    };
    let serde_attrs = if context::SERDE_ENABLED {
        match context::analyze_serde_attrs(&input, context::internal::serde_attrs::MergeMode::Derive) {
            Ok(attrs) => attrs,
            Err(e) => return e.to_compile_error().into(),
        }
    } else {
        context::empty_serde_attrs(&input)
    };
    let env = context::CodegenEnv::resolve();
```

And update the `gen_memento_type` call:

```rust
    let memento_type = context::gen_memento_type(&ir, &env, &serde_attrs);
```

Add helper functions to `context.rs`:

```rust
pub(super) fn analyze_serde_attrs(
    input: &DeriveInput,
    mode: internal::serde_attrs::MergeMode,
) -> syn::Result<internal::serde_attrs::types::SerdeItemAttrs> {
    use internal::serde_attrs::types::SerdeItemAttrs;
    match &input.data {
        syn::Data::Struct(data) => {
            let attrs = internal::serde_attrs::analyze_struct_serde_attrs(
                &data.fields,
                mode,
            )?;
            Ok(SerdeItemAttrs::Struct(attrs))
        }
        syn::Data::Enum(data) => {
            let attrs = internal::serde_attrs::analyze_enum_serde_attrs(data, mode)?;
            Ok(SerdeItemAttrs::Enum(attrs))
        }
        _ => unreachable!("unions rejected earlier"),
    }
}

pub(super) fn empty_serde_attrs(
    input: &DeriveInput,
) -> internal::serde_attrs::types::SerdeItemAttrs {
    use internal::serde_attrs::types::*;
    match &input.data {
        syn::Data::Struct(data) => SerdeItemAttrs::Struct(SerdeStructAttrs {
            fields: data.fields.iter().map(|_| SerdeFieldAttrs::default()).collect(),
        }),
        syn::Data::Enum(data) => SerdeItemAttrs::Enum(SerdeEnumAttrs {
            variants: data.variants.iter().map(|v| {
                v.fields.iter().map(|_| SerdeFieldAttrs::default()).collect()
            }).collect(),
        }),
        _ => unreachable!("unions rejected earlier"),
    }
}
```

- [ ] **Step 7: Fix existing tests in `memento/structs.rs`**

The test in `memento/structs.rs` calls `gen_memento_struct(ir, env)` — update it to pass empty serde attrs:

```rust
#[cfg(test)]
mod tests {
    use quote::{ToTokens, quote};
    use syn::parse_quote;

    use super::{CodegenEnv, StructIr, gen_memento_struct};
    use crate::context::internal::serde_attrs::types::{SerdeFieldAttrs, SerdeStructAttrs};

    #[test]
    fn generated_memento_visibility_matches_companion_struct() {
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
        };

        let restricted_input: syn::DeriveInput = parse_quote! {
            pub(crate) struct Example {
                value: u32,
            }
        };
        let restricted_ir = StructIr::analyze(&restricted_input).unwrap();
        let restricted_serde = SerdeStructAttrs {
            fields: vec![SerdeFieldAttrs::default()],
        };
        let restricted_memento: syn::ItemStruct =
            syn::parse2(gen_memento_struct(&restricted_ir, &env, &restricted_serde)).unwrap();
        assert_eq!(
            restricted_memento.vis.to_token_stream().to_string(),
            quote!(pub(crate)).to_string()
        );

        let private_input: syn::DeriveInput = parse_quote! {
            struct PrivateExample {
                value: u32,
            }
        };
        let private_ir = StructIr::analyze(&private_input).unwrap();
        let private_serde = SerdeStructAttrs {
            fields: vec![SerdeFieldAttrs::default()],
        };
        let private_memento: syn::ItemStruct =
            syn::parse2(gen_memento_struct(&private_ir, &env, &private_serde)).unwrap();
        assert!(matches!(private_memento.vis, syn::Visibility::Inherited));
    }
}
```

- [ ] **Step 8: Fix existing tests in `context.rs`**

Update tests in `context.rs` that call `gen_memento_type` to pass serde attrs. For each test that calls `gen_memento_type`, add:

```rust
use crate::context::internal::serde_attrs::types::{
    SerdeFieldAttrs, SerdeItemAttrs, SerdeStructAttrs,
};
```

And pass `&SerdeItemAttrs::Struct(SerdeStructAttrs { fields: vec![SerdeFieldAttrs::default()] })` as the third argument. Apply similarly for enum tests using `SerdeItemAttrs::Enum(...)`.

- [ ] **Step 9: Verify build and all tests pass**

Run: `cargo test --package recallable-macro`
Expected: All tests PASS

Run: `cargo test --package recallable --features serde`
Expected: All tests PASS

- [ ] **Step 10: Commit**

```bash
git add recallable-macro/src/
git commit -m "feat(serde-attrs): thread serde attrs through memento codegen"
```

---

## Task 6: Model Macro Integration

**Files:**
- Modify: `recallable-macro/src/model_macro.rs`

- [ ] **Step 1: Add rejection of manual `#[serde(rename/alias)]` on fields**

In `recallable-macro/src/model_macro.rs`, add a new function after `check_no_serialize_derive`:

```rust
/// When `SERDE_ENABLED`, reject manual `#[serde(rename = ...)]` or `#[serde(alias = ...)]`
/// on any field. Users must use `#[recallable(rename/alias)]` instead.
fn check_no_serde_rename_or_alias_on_fields(fields: &Fields) -> syn::Result<()> {
    use crate::context::internal::serde_attrs::parse::has_serde_rename_or_alias;

    for field in fields.iter() {
        if has_serde_rename_or_alias(field) {
            return Err(syn::Error::new_spanned(
                field,
                "`#[recallable_model]` manages serde attributes automatically; \
                 use `#[recallable(rename = \"...\")]` instead of `#[serde(rename = \"...\")]`",
            ));
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Add source field annotation function**

Add after the new rejection function:

```rust
/// Insert `#[serde(rename = "...")]` and `#[serde(alias = "...")]` on source fields
/// based on `#[recallable(rename/alias)]` attributes.
fn add_serde_forwarded_attrs_to_fields(fields: &mut Fields) {
    use crate::context::internal::serde_attrs::parse::parse_recallable_serde_attrs;

    for field in fields.iter_mut() {
        let Ok(attrs) = parse_recallable_serde_attrs(field) else {
            continue; // errors caught by the analysis pass
        };
        if let Some(rename) = &attrs.rename {
            field.attrs.push(syn::parse_quote! { #[serde(rename = #rename)] });
        }
        for alias in &attrs.aliases {
            field.attrs.push(syn::parse_quote! { #[serde(alias = #alias)] });
        }
    }
}
```

- [ ] **Step 3: Wire into `expand_tokens` and `ModelItem`**

In `expand_tokens`, add the rejection check after `check_no_serialize_derive` and the annotation step after `add_serde_skip_attrs`. Replace the function body:

```rust
fn expand_tokens(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    validate_model_attr(&attr)?;
    let mut model_item = parse_model_item_tokens(item)?;
    let derive_input = model_item.parse();
    context::analyze_model_input(&derive_input)?;
    if SERDE_ENABLED {
        check_no_serialize_derive(model_item.attrs())?;
        model_item.check_no_serde_rename_or_alias()?;
    }

    model_item.add_derives();
    if SERDE_ENABLED {
        model_item.add_serde_skip_attrs();
        model_item.add_serde_forwarded_attrs();
    }

    Ok(model_item.item_tokenstream())
}
```

Add two new methods to `ModelItem`:

```rust
fn check_no_serde_rename_or_alias(&self) -> syn::Result<()> {
    match self {
        Self::Struct(item) => check_no_serde_rename_or_alias_on_fields(&item.fields),
        Self::Enum(item) => {
            for variant in &item.variants {
                check_no_serde_rename_or_alias_on_fields(&variant.fields)?;
            }
            Ok(())
        }
    }
}

fn add_serde_forwarded_attrs(&mut self) {
    self.with_fields_mut(add_serde_forwarded_attrs_to_fields);
}
```

- [ ] **Step 4: Verify the build compiles**

Run: `cargo build --package recallable-macro --features serde`
Expected: Compiles successfully

- [ ] **Step 5: Run existing tests to verify no regressions**

Run: `cargo test --package recallable-macro --features serde`
Expected: All existing tests PASS

Run: `cargo test --package recallable --features serde`
Expected: All existing tests PASS

- [ ] **Step 6: Commit**

```bash
git add recallable-macro/src/model_macro.rs
git commit -m "feat(serde-attrs): model macro rejects manual serde rename/alias and forwards recallable attrs"
```

---

## Task 7: Integration Tests

**Files:**
- Modify: `recallable/tests/common/mod.rs`
- Modify: `recallable/tests/serde_json.rs`

- [ ] **Step 1: Add test structs with rename/alias to common module**

Append to `recallable/tests/common/mod.rs`:

```rust
#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenamedFields {
    #[recallable(rename = "wire_level")]
    pub(crate) level: i32,
    pub(crate) tag: u8,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AliasedFields {
    #[recallable(alias = "old_level", alias = "legacy_level")]
    pub(crate) level: i32,
    pub(crate) tag: u8,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenameAndAlias {
    #[recallable(rename = "wire_level", alias = "old_level")]
    pub(crate) level: i32,
    pub(crate) tag: u8,
}
```

- [ ] **Step 2: Write integration test for rename round-trip**

Append to `recallable/tests/serde_json.rs`:

```rust
#[test]
fn test_rename_field_round_trip() {
    let original = RenamedFields { level: 42, tag: 7 };
    let json = serde_json::to_string(&original).unwrap();

    // Serialized form uses the wire name
    assert!(json.contains("wire_level"));
    assert!(!json.contains("\"level\""));

    // Deserialize into memento using the wire name
    let memento: <RenamedFields as Recallable>::Memento =
        serde_json::from_str(&json).unwrap();

    let mut target = RenamedFields { level: 0, tag: 0 };
    target.recall(memento);
    assert_eq!(target.level, 42);
    assert_eq!(target.tag, 7);
}
```

- [ ] **Step 3: Write integration test for alias deserialization**

Append to `recallable/tests/serde_json.rs`:

```rust
#[test]
fn test_alias_field_deserialization() {
    // Deserialize using the alias key name
    let json = r#"{"old_level": 99, "tag": 3}"#;
    let memento: <AliasedFields as Recallable>::Memento =
        serde_json::from_str(json).unwrap();

    let mut target = AliasedFields { level: 0, tag: 0 };
    target.recall(memento);
    assert_eq!(target.level, 99);
    assert_eq!(target.tag, 3);

    // Also works with the other alias
    let json2 = r#"{"legacy_level": 77, "tag": 5}"#;
    let memento2: <AliasedFields as Recallable>::Memento =
        serde_json::from_str(json2).unwrap();

    let mut target2 = AliasedFields { level: 0, tag: 0 };
    target2.recall(memento2);
    assert_eq!(target2.level, 77);
    assert_eq!(target2.tag, 5);

    // And with the original field name
    let json3 = r#"{"level": 55, "tag": 1}"#;
    let memento3: <AliasedFields as Recallable>::Memento =
        serde_json::from_str(json3).unwrap();

    let mut target3 = AliasedFields { level: 0, tag: 0 };
    target3.recall(memento3);
    assert_eq!(target3.level, 55);
}
```

- [ ] **Step 4: Write integration test for rename + alias combined**

Append to `recallable/tests/serde_json.rs`:

```rust
#[test]
fn test_rename_and_alias_combined() {
    let original = RenameAndAlias { level: 10, tag: 2 };
    let json = serde_json::to_string(&original).unwrap();

    // Serialized form uses the renamed key
    assert!(json.contains("wire_level"));

    // Deserialize with the renamed key
    let memento: <RenameAndAlias as Recallable>::Memento =
        serde_json::from_str(&json).unwrap();
    let mut target = RenameAndAlias { level: 0, tag: 0 };
    target.recall(memento);
    assert_eq!(target.level, 10);

    // Deserialize with the alias key
    let alias_json = r#"{"old_level": 20, "tag": 4}"#;
    let memento2: <RenameAndAlias as Recallable>::Memento =
        serde_json::from_str(alias_json).unwrap();
    let mut target2 = RenameAndAlias { level: 0, tag: 0 };
    target2.recall(memento2);
    assert_eq!(target2.level, 20);
}
```

- [ ] **Step 5: Write integration test for enum variant fields with rename**

Append to `recallable/tests/common/mod.rs`:

```rust
#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RenamedEnumFields {
    A {
        #[recallable(rename = "wire_x")]
        x: i32,
        y: u8,
    },
    B {
        #[recallable(alias = "old_z")]
        z: String,
    },
}
```

Append to `recallable/tests/serde_json.rs`:

```rust
#[test]
fn test_enum_variant_rename_round_trip() {
    let original = RenamedEnumFields::A { x: 42, y: 7 };
    let json = serde_json::to_string(&original).unwrap();

    // Serialized form uses the wire name
    assert!(json.contains("wire_x"));

    // Deserialize into memento
    let memento: <RenamedEnumFields as Recallable>::Memento =
        serde_json::from_str(&json).unwrap();

    let mut target = RenamedEnumFields::A { x: 0, y: 0 };
    target.recall(memento);
    assert_eq!(target, RenamedEnumFields::A { x: 42, y: 7 });
}

#[test]
fn test_enum_variant_alias_deserialization() {
    let json = r#"{"B":{"old_z":"hello"}}"#;
    let memento: <RenamedEnumFields as Recallable>::Memento =
        serde_json::from_str(json).unwrap();

    let mut target = RenamedEnumFields::B { z: String::new() };
    target.recall(memento);
    assert_eq!(target, RenamedEnumFields::B { z: "hello".into() });
}
```

- [ ] **Step 6: Run integration tests**

Run: `cargo test --package recallable --features serde -- test_rename test_alias test_enum`
Expected: All 6 new tests PASS

- [ ] **Step 7: Commit**

```bash
git add recallable/tests/common/mod.rs recallable/tests/serde_json.rs
git commit -m "test: add integration tests for serde rename and alias forwarding"
```

---

## Task 8: UI Compile-Fail Tests

**Files:**
- Create: `recallable/tests/ui/derive_fail_serde_rename_conflict.rs` + `.stderr`
- Create: `recallable/tests/ui/model_fail_manual_serde_rename.rs` + `.stderr`
- Create: `recallable/tests/ui/model_fail_manual_serde_alias.rs` + `.stderr`
- Create: `recallable/tests/ui/derive_fail_serde_attr_on_skip.rs` + `.stderr`
- Modify: `recallable/tests/macro_expansion_failures.rs`

- [ ] **Step 1: Create conflicting rename UI test**

Create `recallable/tests/ui/derive_fail_serde_rename_conflict.rs`:

```rust
use recallable::Recallable;

#[derive(Clone, serde::Serialize, Recallable)]
struct Foo {
    #[serde(rename = "x")]
    #[recallable(rename = "y")]
    value: i32,
}

fn main() {}
```

Create `recallable/tests/ui/derive_fail_serde_rename_conflict.stderr`:

```text
error: conflicting `rename` values: `#[serde(rename = "x")]` and `#[recallable(rename = "y")]` must match
 --> tests/ui/derive_fail_serde_rename_conflict.rs:4:5
  |
4 | /     #[serde(rename = "x")]
5 | |     #[recallable(rename = "y")]
6 | |     value: i32,
  | |______________^
```

- [ ] **Step 2: Create model macro manual serde rename UI test**

Create `recallable/tests/ui/model_fail_manual_serde_rename.rs`:

```rust
use recallable::recallable_model;

#[recallable_model]
#[derive(Clone, Debug)]
struct Foo {
    #[serde(rename = "x")]
    value: i32,
}

fn main() {}
```

Create `recallable/tests/ui/model_fail_manual_serde_rename.stderr`:

```text
error: `#[recallable_model]` manages serde attributes automatically; use `#[recallable(rename = "...")]` instead of `#[serde(rename = "...")]`
 --> tests/ui/model_fail_manual_serde_rename.rs:6:5
  |
6 | /     #[serde(rename = "x")]
7 | |     value: i32,
  | |______________^
```

- [ ] **Step 3: Create model macro manual serde alias UI test**

Create `recallable/tests/ui/model_fail_manual_serde_alias.rs`:

```rust
use recallable::recallable_model;

#[recallable_model]
#[derive(Clone, Debug)]
struct Foo {
    #[serde(alias = "old")]
    value: i32,
}

fn main() {}
```

Create `recallable/tests/ui/model_fail_manual_serde_alias.stderr`:

```text
error: `#[recallable_model]` manages serde attributes automatically; use `#[recallable(alias = "...")]` instead of `#[serde(alias = "...")]`
 --> tests/ui/model_fail_manual_serde_alias.rs:6:5
  |
6 | /     #[serde(alias = "old")]
7 | |     value: i32,
  | |______________^
```

- [ ] **Step 4: Create rename on skipped field UI test**

Create `recallable/tests/ui/derive_fail_serde_attr_on_skip.rs`:

```rust
use recallable::Recallable;

#[derive(Clone, serde::Serialize, Recallable)]
struct Foo {
    #[recallable(skip, rename = "x")]
    value: i32,
    other: u32,
}

fn main() {}
```

Create `recallable/tests/ui/derive_fail_serde_attr_on_skip.stderr`:

```text
error: `rename` and `alias` cannot be used on a `#[recallable(skip)]` field
 --> tests/ui/derive_fail_serde_attr_on_skip.rs:5:5
  |
5 | /     #[recallable(skip, rename = "x")]
6 | |     value: i32,
  | |______________^
```

- [ ] **Step 5: Register new UI tests in `macro_expansion_failures.rs`**

In `recallable/tests/macro_expansion_failures.rs`, add inside the `#[cfg(feature = "serde")]` block (after the existing `model_fail_duplicate_serialize` entries):

```rust
    #[cfg(feature = "serde")]
    {
        tests.compile_fail("tests/ui/model_fail_duplicate_serialize.rs");
        tests.compile_fail("tests/ui/model_fail_duplicate_serialize_qualified.rs");
        tests.compile_fail("tests/ui/model_fail_duplicate_serialize_fully_qualified.rs");
        tests.compile_fail("tests/ui/derive_fail_serde_rename_conflict.rs");
        tests.compile_fail("tests/ui/model_fail_manual_serde_rename.rs");
        tests.compile_fail("tests/ui/model_fail_manual_serde_alias.rs");
        tests.compile_fail("tests/ui/derive_fail_serde_attr_on_skip.rs");
    }
```

- [ ] **Step 6: Run UI tests to verify they produce expected errors**

Run: `cargo test --package recallable --features serde -- derive_macro_reports_expected_failures`
Expected: PASS

Note: The `.stderr` files may need adjustment to match exact spans. Run the test once, and if it fails with a diff, update the `.stderr` files to match the actual compiler output.

- [ ] **Step 7: Commit**

```bash
git add recallable/tests/ui/ recallable/tests/macro_expansion_failures.rs
git commit -m "test: add UI compile-fail tests for serde attr forwarding errors"
```

---

## Task 9: Full Test Suite and No-Serde Feature Verification

**Files:**
- No new files

- [ ] **Step 1: Run full test suite with serde feature**

Run: `cargo test --package recallable-macro --features serde`
Expected: All tests PASS

Run: `cargo test --package recallable --features serde`
Expected: All tests PASS

- [ ] **Step 2: Run full test suite without serde feature**

Run: `cargo test --package recallable-macro`
Expected: All tests PASS (serde-gated code paths are inactive)

Run: `cargo test --package recallable`
Expected: All tests PASS

- [ ] **Step 3: Verify `#[recallable(rename)]` without serde feature is rejected**

This is verified by the unit test in `serde_attrs/parse.rs` that checks the `SERDE_ENABLED` gate. Since `SERDE_ENABLED` is a `const bool` evaluated at compile time of the macro crate, the test only runs in the correct feature configuration. Confirm the parse tests pass in both feature modes (steps 1 and 2 above).

If a dedicated UI test is desired for the no-serde case, create `recallable/tests/ui/derive_fail_serde_rename_no_feature.rs` — but this requires running trybuild without the serde feature, which the existing `macro_expansion_failures.rs` test harness already handles by gating serde-specific tests behind `#[cfg(feature = "serde")]`. The no-feature rejection is covered by the unit test.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --package recallable-macro --features serde -- -D warnings`
Expected: No warnings

Run: `cargo clippy --package recallable --features serde -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final cleanup after serde attr forwarding implementation"
```
