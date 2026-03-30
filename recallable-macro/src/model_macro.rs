use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::{Fields, Item, ItemEnum, ItemStruct, parse_quote};

use crate::context::{self, SERDE_ENABLED, crate_path, has_recallable_skip_attr};

const DERIVE: &str = "derive";
const SERIALIZE: &str = "Serialize";
const SERDE: &str = "serde";
const SERDE_DERIVE: &str = "serde_derive";

#[must_use]
pub(super) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_tokens(attr.into(), item.into()) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

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

fn validate_model_attr(attr: &TokenStream2) -> syn::Result<()> {
    if attr.is_empty() {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            attr,
            "`#[recallable_model]` does not accept arguments",
        ))
    }
}

#[must_use]
fn build_model_derive_attr(crate_path: &TokenStream2) -> syn::Attribute {
    if SERDE_ENABLED {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall, ::serde::Serialize)]
        }
    } else {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall)]
        }
    }
}

fn parse_model_item_tokens(item: TokenStream2) -> syn::Result<ModelItem> {
    let item: Item = syn::parse2(item)?;
    match item {
        Item::Struct(item) => Ok(ModelItem::Struct(item)),
        Item::Enum(item) => Ok(ModelItem::Enum(item)),
        other => Err(syn::Error::new_spanned(
            other,
            "`#[recallable_model]` can only be applied to structs or enums",
        )),
    }
}

fn add_serde_skip_attrs_to_fields(fields: &mut Fields) {
    fields
        .iter_mut()
        .filter(|field| has_recallable_skip_attr(field))
        .for_each(|field| field.attrs.push(parse_quote! { #[serde(skip)] }));
}

fn check_no_serde_rename_or_alias_on_fields(fields: &Fields) -> syn::Result<()> {
    for field in fields.iter() {
        if context::has_serde_rename_or_alias(field) {
            return Err(syn::Error::new_spanned(
                field,
                "`#[recallable_model]` manages serde attributes automatically; \
                 use `#[recallable(rename = \"...\")]` instead of `#[serde(rename = \"...\")]`",
            ));
        }
    }
    Ok(())
}

fn add_serde_forwarded_attrs_to_fields(fields: &mut Fields) {
    for field in fields.iter_mut() {
        let Ok(attrs) = context::parse_recallable_serde_attrs(field) else {
            continue;
        };
        if let Some(rename) = &attrs.rename {
            field.attrs.push(parse_quote! { #[serde(rename = #rename)] });
        }
        for alias in &attrs.aliases {
            field.attrs.push(parse_quote! { #[serde(alias = #alias)] });
        }
    }
}

/// Returns an error if any existing `#[derive(...)]` attribute on the struct
/// already includes a serde-backed `Serialize` derive.
///
/// Called only when `SERDE_ENABLED` is true, before `#[recallable_model]`
/// injects its own `::serde::Serialize` derive.
fn check_no_serialize_derive(attrs: &[syn::Attribute]) -> syn::Result<()> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident(DERIVE))
        .try_for_each(|attr| {
            attr.parse_nested_meta(|meta| {
                if is_serde_serialize_path(&meta.path) {
                    Err(meta.error(
                        "`#[recallable_model]` already derives `serde::Serialize` when the \
                         `serde` feature is enabled — remove the manual `Serialize` derive",
                    ))
                } else {
                    Ok(())
                }
            })
        })
}

fn is_serde_serialize_path(path: &syn::Path) -> bool {
    // Attribute macros cannot resolve imported names, so keep treating a bare
    // `Serialize` derive as serde-shaped for the common `use serde::Serialize;` case.
    path.is_ident("Serialize") || {
        let mut segments = path.segments.iter();
        matches!(
            (segments.next(), segments.next(), segments.next()),
            (Some(first), Some(second), None)
                if (first.ident == SERDE || first.ident == SERDE_DERIVE)
                    && second.ident == SERIALIZE
        )
    }
}

enum ModelItem {
    Struct(ItemStruct),
    Enum(ItemEnum),
}

impl ModelItem {
    fn attrs(&self) -> &[syn::Attribute] {
        match self {
            Self::Struct(item) => &item.attrs,
            Self::Enum(item) => &item.attrs,
        }
    }

    fn attrs_mut(&mut self) -> &mut Vec<syn::Attribute> {
        match self {
            Self::Struct(item) => &mut item.attrs,
            Self::Enum(item) => &mut item.attrs,
        }
    }

    fn with_fields_mut(&mut self, mut apply: impl FnMut(&mut Fields)) {
        match self {
            Self::Struct(item) => apply(&mut item.fields),
            Self::Enum(item) => item
                .variants
                .iter_mut()
                .for_each(|variant| apply(&mut variant.fields)),
        }
    }

    fn add_derives(&mut self) {
        let crate_path = crate_path();
        let derives = build_model_derive_attr(&crate_path);
        self.attrs_mut().push(derives);
    }

    fn add_serde_skip_attrs(&mut self) {
        self.with_fields_mut(add_serde_skip_attrs_to_fields);
    }

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

    fn item_tokenstream(&self) -> TokenStream2 {
        match self {
            ModelItem::Struct(item) => item.to_token_stream(),
            ModelItem::Enum(item) => item.to_token_stream(),
        }
    }

    fn parse(&self) -> syn::DeriveInput {
        match self {
            ModelItem::Struct(item) => item.clone().into(),
            ModelItem::Enum(item) => item.clone().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::{
        expand_tokens, is_serde_serialize_path, parse_model_item_tokens, validate_model_attr,
    };

    #[test]
    fn serde_serialize_path_detection_is_precise() {
        assert!(is_serde_serialize_path(&parse_quote!(Serialize)));
        assert!(is_serde_serialize_path(&parse_quote!(serde::Serialize)));
        assert!(is_serde_serialize_path(&parse_quote!(::serde::Serialize)));
        assert!(is_serde_serialize_path(&parse_quote!(
            serde_derive::Serialize
        )));
        assert!(is_serde_serialize_path(&parse_quote!(
            ::serde_derive::Serialize
        )));

        assert!(!is_serde_serialize_path(&parse_quote!(other::Serialize)));
        assert!(!is_serde_serialize_path(&parse_quote!(
            serde::ser::Serialize
        )));
        assert!(!is_serde_serialize_path(&parse_quote!(
            other::serde::Serialize
        )));
        assert!(!is_serde_serialize_path(&parse_quote!(SerializeOwned)));
    }

    #[test]
    fn recallable_model_rejects_arguments() {
        let error = validate_model_attr(&quote!(unexpected)).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`#[recallable_model]` does not accept arguments")
        );
    }

    #[test]
    fn parse_model_item_rejects_non_struct_or_enum_items() {
        let error = match parse_model_item_tokens(quote!(
            fn example() {}
        )) {
            Ok(_) => panic!("expected parse_model_item to reject functions"),
            Err(error) => error,
        };

        assert_eq!(
            error.to_string(),
            "`#[recallable_model]` can only be applied to structs or enums"
        );
    }

    #[test]
    fn expand_tokens_reject_model_arguments() {
        let error = expand_tokens(
            quote!(unexpected),
            quote!(
                struct Example;
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("does not accept arguments"));
    }

    #[test]
    fn expand_tokens_reject_non_model_items() {
        let error = expand_tokens(
            quote!(),
            quote!(
                fn example() {}
            ),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("can only be applied to structs or enums")
        );
    }

    #[test]
    fn expand_tokens_reject_model_analysis_failures() {
        let error = expand_tokens(
            quote!(),
            quote! {
                enum Example {
                    Value(#[recallable] Inner),
                }
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("assignment-only variants"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn expand_tokens_reject_manual_serialize_derives() {
        let error = expand_tokens(
            quote!(),
            quote! {
                #[derive(serde::Serialize)]
                struct Example {
                    value: u32,
                }
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("already derives"));
    }
}
