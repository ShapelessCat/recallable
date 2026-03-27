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
//!   recalls fields annotated with `#[recallable]`.
//!
//! Feature flags are evaluated in the `recallable-macro` crate itself. See `context`
//! for details about the generated memento struct and trait implementations.

use proc_macro::TokenStream;

use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod context;
mod model_macro;

/// Attribute macro that augments a struct with `Recallable`/`Recall` derives.
///
/// - Always adds `#[derive(Recallable, Recall)]`.
/// - When the `serde` feature is enabled for the macro crate, it also adds
///   `#[derive(serde::Serialize)]`.
/// - For fields annotated with `#[recallable(skip)]`, it injects `#[serde(skip)]`
///   to keep serde output aligned with recall behavior.
/// - This attribute itself takes no arguments.
///
/// This macro preserves the original struct shape and only mutates attributes.
///
/// **Attribute ordering:** This macro must appear *before* any attributes it needs
/// to inspect. An attribute macro only receives attributes that follow it in source
/// order. For example, `#[derive(Serialize)]` placed above `#[recallable_model]` is
/// invisible to the macro and will cause a duplicate-derive error.
#[proc_macro_attribute]
pub fn recallable_model(attr: TokenStream, item: TokenStream) -> TokenStream {
    model_macro::expand(attr, item)
}

/// Derive macro that generates the companion memento type and `Recallable` impl.
///
/// The generated memento type:
/// - mirrors the original struct shape (named/tuple/unit),
/// - includes fields unless marked with `#[recallable(skip)]`,
/// - uses the same visibility as the input struct,
/// - keeps all generated fields private by omitting field-level visibility modifiers,
/// - also derives `serde::Deserialize` when the `serde` feature is enabled for the
///   macro crate.
///
/// For `#[recallable]` fields, the generated memento field type is exactly
/// `<FieldType as Recallable>::Memento`. The macro does not prescribe one canonical container
/// semantics; it uses whatever memento shape the field type defines.
///
/// The companion struct itself is generated as an internal implementation detail. The supported
/// way to name it is `<Struct as Recallable>::Memento`. It is intended to be produced and consumed
/// alongside the source struct, primarily through `Recall::recall`/`TryRecall::try_recall`, not as
/// a field-inspection surface with widened visibility.
///
/// The `Recallable` impl sets `type Memento` to that generated type and adds any required generic
/// bounds.
///
/// The generated memento struct always derives `Clone`, `Debug`, and `PartialEq`.
/// When the `serde` feature is enabled, it also derives `serde::Deserialize`.
/// All non-skipped field types must implement these derived traits.
///
/// To suppress the default `Clone`, `Debug`, and `PartialEq` derives (and their
/// corresponding trait bounds), annotate the struct with
/// `#[recallable(skip_memento_default_derives)]`. When serde is enabled, `Deserialize` is
/// still derived on the memento even with this attribute.
///
/// When the `impl_from` feature is enabled for the macro crate, a
/// `From<Struct>` implementation is also generated for the memento type. For `#[recallable]`
/// fields, that additionally requires `<FieldType as Recallable>::Memento: From<FieldType>`.
#[proc_macro_derive(Recallable, attributes(recallable))]
pub fn derive_recallable(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let ir = match context::ItemIr::analyze(&input) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error().into(),
    };
    let env = context::CodegenEnv::resolve();

    let memento_struct = context::gen_memento_type(&ir, &env);
    let recallable_impl = context::gen_recallable_impl(&ir, &env);
    let from_impl = if context::IMPL_FROM_ENABLED {
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
///
/// For `#[recallable]` fields, replace/merge behavior comes from the field type's own
/// `Recall` implementation.
pub fn derive_recall(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let ir = match context::ItemIr::analyze(&input) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error().into(),
    };
    if let context::ItemIr::Enum(enum_ir) = &ir
        && let Err(e) = enum_ir.ensure_recall_derivable()
    {
        return e.to_compile_error().into();
    }
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
