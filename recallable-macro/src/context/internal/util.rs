use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Attribute, Ident};

const RECALLABLE: &str = "recallable";

/// Returns the path used to reference the `recallable` crate in generated code.
///
/// Uses `proc-macro-crate` to resolve the actual dependency name from `Cargo.toml`,
/// which handles crate renames (e.g., `my_recallable = { package = "recallable", ... }`).
///
/// Even when the macro expands inside the `recallable` crate itself, prefer the
/// absolute `::recallable` path instead of `crate`. That keeps doctests working:
/// rustdoc compiles them as external crates, so `crate` would point at the
/// temporary doctest crate rather than the real `recallable` library.
#[inline]
pub(crate) fn crate_path() -> TokenStream2 {
    match crate_name(RECALLABLE) {
        Ok(FoundCrate::Itself) => quote! { ::recallable },
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote! { ::#ident }
        }
        Err(_) => quote! { ::recallable },
    }
}

#[inline]
pub(super) fn is_recallable_attr(attr: &Attribute) -> bool {
    attr.path().is_ident(RECALLABLE)
}
