//! # Recallable Macro
//!
//! Procedural macros backing the `recallable` crate.
//!
//! Provided macros:
//!
//! - `#[recallable_model]`: injects `Recallable`/`Recall` derives; with the `serde`
//!   Cargo feature enabled for this macro crate it also adds `serde::Serialize`
//!   and applies `#[serde(skip)]` to fields marked `#[recallable(skip)]`.
//!
//! - `#[derive(Recallable)]`: generates the companion `<Struct>Memento` type and the
//!   `Recallable` impl; with the `impl_from` Cargo feature it also generates
//!   `From<Struct>` for the memento type.
//!
//! - `#[derive(Recall)]`: generates the `Recall` implementation and recursively
//!   recalles fields annotated with `#[recallable]`.
//!
//! Feature flags are evaluated in the `recallable-macro` crate itself. See `context`
//! for details about the generated memento struct and trait implementations.

use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, ItemStruct, parse_macro_input, parse_quote};

mod context;

use syn::DeriveInput;

use crate::context::{IS_SERDE_ENABLED, crate_path, has_recallable_skip_attr};

const IS_IMPL_FROM_ENABLED: bool = cfg!(feature = "impl_from");

#[proc_macro_attribute]
/// Attribute macro that augments a struct with `Recallable`/`Recall` derives.
///
/// - Always adds `#[derive(Recallable, Recall)]`.
/// - When the `serde` feature is enabled for the macro crate, it also adds
///   `#[derive(serde::Serialize)]`.
/// - For fields annotated with `#[recallable(skip)]`, it injects `#[serde(skip)]`
///   to keep serde output aligned with recalling behavior.
///
/// This macro preserves the original struct shape and only mutates attributes.
pub fn recallable_model(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_path = crate_path();
    let derives = if IS_SERDE_ENABLED {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall, ::serde::Serialize)]
        }
    } else {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall)]
        }
    };

    let mut input = parse_macro_input!(item as ItemStruct);
    input.attrs.push(derives);

    if IS_SERDE_ENABLED {
        add_serde_skip_attrs(&mut input.fields);
    }

    (quote! { #input }).into()
}

#[proc_macro_derive(Recallable, attributes(recallable))]
/// Derive macro that generates the companion memento type and `Recallable` impl.
///
/// The generated memento type:
/// - mirrors the original struct shape (named/tuple/unit),
/// - includes fields unless marked with `#[recallable(skip)]`,
/// - also derives `serde::Deserialize` when the `serde` feature is enabled for the
///   macro crate.
///
/// The `Recallable` impl sets `type Memento = <StructName>Memento<...>` and adds
/// any required generic bounds.
///
/// When the `impl_from` feature is enabled for the macro crate, a
/// `From<Struct>` implementation is also generated for the memento type.
pub fn derive_recallable(input: TokenStream) -> TokenStream {
    expand(input, |ctx| {
        let memento_struct_def = ctx.build_memento_struct();
        let recallable_trait_impl = ctx.build_recallable_trait_impl();
        let from_struct_impl = IS_IMPL_FROM_ENABLED.then(|| {
            let from_struct_impl = ctx.build_from_trait_impl();
            quote! {
                #[automatically_derived]
                #from_struct_impl
            }
        });

        quote! {
            const _: () = {
                #[automatically_derived]
                #memento_struct_def

                #[automatically_derived]
                #recallable_trait_impl

                #from_struct_impl
            };
        }
    })
}

#[proc_macro_derive(Recall, attributes(recallable))]
/// Derive macro that generates the `Recall` trait implementation.
///
/// The generated `recall` method:
/// - assigns fields directly by default,
/// - recursively calls `recall` on fields marked with `#[recallable]`,
/// - respects `#[recallable(skip)]` by omitting those fields from recalling.
pub fn derive_recall(input: TokenStream) -> TokenStream {
    expand(input, |ctx| {
        let recall_trait_impl = ctx.build_recall_trait_impl();

        quote! {
            const _: () = {
                #[automatically_derived]
                #recall_trait_impl
            };
        }
    })
}

fn expand<F>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(&context::MacroContext) -> TokenStream2,
{
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    match context::MacroContext::new(&input) {
        Ok(ctx) => f(&ctx).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn add_serde_skip_attrs(fields: &mut Fields) {
    for field in fields.iter_mut() {
        if has_recallable_skip_attr(field) {
            field.attrs.push(parse_quote! { #[serde(skip)] });
        }
    }
}
