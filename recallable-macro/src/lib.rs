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
//! - `#[derive(Recallable)]`: generates an internal companion memento struct, exposes
//!   it as `<Struct as Recallable>::Memento`, and emits the `Recallable` impl; with the
//!   `impl_from` Cargo feature it also generates `From<Struct>` for the memento type.
//!
//! - `#[derive(Recall)]`: generates the `Recall` implementation and recursively
//!   recalles fields annotated with `#[recallable]`.
//!
//! Feature flags are evaluated in the `recallable-macro` crate itself. See `context`
//! for details about the generated memento struct and trait implementations.

use proc_macro::TokenStream;

use quote::quote;
use syn::{DeriveInput, Fields, ItemStruct, parse_macro_input, parse_quote};

mod context;

use crate::context::{IS_SERDE_ENABLED, crate_path, has_recallable_skip_attr};

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
///
/// **Attribute ordering:** This macro must appear *before* any attributes it needs
/// to inspect. An attribute macro only receives attributes that follow it in source
/// order. For example, `#[derive(Serialize)]` placed above `#[recallable_model]` is
/// invisible to the macro and will cause a duplicate-derive error.
pub fn recallable_model(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_path = crate_path();
    let mut input = parse_macro_input!(item as ItemStruct);

    if IS_SERDE_ENABLED && let Err(e) = check_no_serialize_derive(&input.attrs) {
        return e.to_compile_error().into();
    }

    let derives = if IS_SERDE_ENABLED {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall, ::serde::Serialize)]
        }
    } else {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall)]
        }
    };

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
/// The companion struct itself is generated as an internal implementation detail. The supported
/// way to name it is `<Struct as Recallable>::Memento`.
///
/// The `Recallable` impl sets `type Memento` to that generated type and adds any required generic
/// bounds.
///
/// The generated memento struct always derives `Clone`, `Debug`, and `PartialEq`.
/// When the `serde` feature is enabled, it also derives `serde::Deserialize`.
/// All non-skipped field types must implement these traits.
///
/// When the `impl_from` feature is enabled for the macro crate, a
/// `From<Struct>` implementation is also generated for the memento type.
pub fn derive_recallable(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let ir = match context::StructIr::analyze(&input) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error().into(),
    };
    let env = context::CodegenEnv::resolve();

    let memento_struct = context::gen_memento_struct(&ir, &env);
    let recallable_impl = context::gen_recallable_impl(&ir, &env);
    let from_impl = if env.impl_from_enabled {
        let from_impl = context::gen_from_impl(&ir, &env);
        quote! {
            #[automatically_derived]
            #from_impl
        }
    } else {
        quote! {}
    };

    let output = quote! {
        const _: () = {
            #[automatically_derived]
            #memento_struct

            #[automatically_derived]
            #recallable_impl

            #from_impl
        };
    };
    output.into()
}

#[proc_macro_derive(Recall, attributes(recallable))]
/// Derive macro that generates the `Recall` trait implementation.
///
/// The generated `recall` method:
/// - assigns fields directly by default,
/// - recursively calls `recall` on fields marked with `#[recallable]`,
/// - respects `#[recallable(skip)]` by omitting those fields from recalling.
pub fn derive_recall(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let ir = match context::StructIr::analyze(&input) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error().into(),
    };
    let env = context::CodegenEnv::resolve();

    let recall_impl = context::gen_recall_impl(&ir, &env);

    let output = quote! {
        const _: () = {
            #[automatically_derived]
            #recall_impl
        };
    };
    output.into()
}

fn add_serde_skip_attrs(fields: &mut Fields) {
    for field in fields.iter_mut() {
        if has_recallable_skip_attr(field) {
            field.attrs.push(parse_quote! { #[serde(skip)] });
        }
    }
}

/// Returns an error if any existing `#[derive(...)]` attribute on the struct
/// already includes a path whose last segment is `Serialize`.
///
/// Called only when `IS_SERDE_ENABLED` is true, before `#[recallable_model]`
/// injects its own `::serde::Serialize` derive.
fn check_no_serialize_derive(attrs: &[syn::Attribute]) -> syn::Result<()> {
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta
                .path
                .segments
                .last()
                .map(|s| s.ident == "Serialize")
                .unwrap_or(false)
            {
                return Err(meta.error(
                    "`#[recallable_model]` already derives `serde::Serialize` when the \
                     `serde` feature is enabled — remove the manual `#[derive(Serialize)]`",
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}
