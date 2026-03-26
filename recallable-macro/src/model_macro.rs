use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, ItemStruct, parse_macro_input, parse_quote};

use crate::context::{SERDE_ENABLED, crate_path, has_recallable_skip_attr};

pub(super) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Err(err) = validate_model_attr(attr.into()) {
        return err.to_compile_error().into();
    }

    let crate_path = crate_path();
    let mut input = parse_macro_input!(item as ItemStruct);

    if SERDE_ENABLED && let Err(e) = check_no_serialize_derive(&input.attrs) {
        return e.to_compile_error().into();
    }

    let derives = if SERDE_ENABLED {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall, ::serde::Serialize)]
        }
    } else {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall)]
        }
    };

    input.attrs.push(derives);

    if SERDE_ENABLED {
        add_serde_skip_attrs(&mut input.fields);
    }

    (quote! { #input }).into()
}

fn validate_model_attr(attr: TokenStream2) -> syn::Result<()> {
    syn::parse2::<syn::parse::Nothing>(attr.clone())
        .map(|_| ())
        .map_err(|_| {
            syn::Error::new_spanned(attr, "`#[recallable_model]` does not accept arguments")
        })
}

fn add_serde_skip_attrs(fields: &mut Fields) {
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
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if is_serde_serialize_path(&meta.path) {
                return Err(meta.error(
                    "`#[recallable_model]` already derives `serde::Serialize` when the \
                     `serde` feature is enabled — remove the manual `Serialize` derive",
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn is_serde_serialize_path(path: &syn::Path) -> bool {
    if path.is_ident("Serialize") {
        // Attribute macros cannot resolve imported names, so keep treating a bare
        // `Serialize` derive as serde-shaped for the common `use serde::Serialize;` case.
        return true;
    }

    let mut segments = path.segments.iter();
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(first), Some(second), None)
            if (first.ident == "serde" || first.ident == "serde_derive")
                && second.ident == "Serialize"
    )
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
        let error = validate_model_attr(quote!(unexpected)).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`#[recallable_model]` does not accept arguments")
        );
    }
}
