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
    let attr_tokens: TokenStream2 = attr.into();
    if let Err(err) = validate_model_attr(&attr_tokens) {
        return err.to_compile_error().into();
    }

    let mut model_item = match parse_model_item(item) {
        Ok(item) => item,
        Err(e) => return e.to_compile_error().into(),
    };
    let derive_input: syn::DeriveInput = match model_item.parse() {
        Ok(input) => input,
        Err(e) => return e.to_compile_error().into(),
    };
    if let Err(e) = context::analyze_model_input(&derive_input) {
        return e.to_compile_error().into();
    }
    if SERDE_ENABLED && let Err(e) = check_no_serialize_derive(model_item.attrs()) {
        return e.to_compile_error().into();
    }

    model_item.add_derives();

    if SERDE_ENABLED {
        model_item.add_serde_skip_attrs();
    }

    model_item.item_tokenstream().into()
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

fn parse_model_item(item: TokenStream) -> syn::Result<ModelItem> {
    let item: Item = syn::parse(item)?;
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
    for field in fields.iter_mut() {
        if has_recallable_skip_attr(field) {
            field.attrs.push(parse_quote! { #[serde(skip)] });
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

    fn add_derives(&mut self) {
        let crate_path = crate_path();
        let derives = build_model_derive_attr(&crate_path);
        let attrs = match self {
            Self::Struct(item) => &mut item.attrs,
            Self::Enum(item) => &mut item.attrs,
        };
        attrs.push(derives);
    }

    fn add_serde_skip_attrs(&mut self) {
        match self {
            Self::Struct(item) => add_serde_skip_attrs_to_fields(&mut item.fields),
            Self::Enum(item) => {
                for variant in &mut item.variants {
                    add_serde_skip_attrs_to_fields(&mut variant.fields);
                }
            }
        }
    }

    fn item_tokenstream(&self) -> TokenStream2 {
        match self {
            ModelItem::Struct(item) => item.to_token_stream(),
            ModelItem::Enum(item) => item.to_token_stream(),
        }
    }

    fn parse(&self) -> syn::Result<syn::DeriveInput> {
        syn::parse2(self.item_tokenstream())
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::{is_serde_serialize_path, validate_model_attr};

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
}
