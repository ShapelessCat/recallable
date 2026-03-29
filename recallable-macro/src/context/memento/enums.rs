use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Ident, WhereClause, WherePredicate};

use crate::context::SERDE_ENABLED;
use crate::context::internal::enums::{
    EnumIr, VariantIr, VariantShape, collect_recall_like_bounds_for_enum,
};
use crate::context::internal::shared::{
    CodegenEnv, CodegenItemIr, FieldIr, build_memento_field_tokens,
};

#[must_use]
pub(crate) fn gen_memento_enum(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let derives = ir.memento_trait_spec().derive_attr();
    let marker_helpers = ir.synthetic_marker_helper_defs();
    let visibility = ir.visibility();
    let memento_name = ir.memento_name();
    let memento_generics = ir.memento_decl_generics();
    let where_clause = build_memento_where_clause(ir, env);
    let variants = ir
        .variants()
        .map(|variant| {
            build_memento_variant(
                variant,
                &env.recallable_trait,
                ir.generic_type_param_idents(),
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

fn build_memento_where_clause(ir: &EnumIr, env: &CodegenEnv) -> Option<WhereClause> {
    let mut where_clause = ir
        .memento_where_clause()
        .cloned()
        .unwrap_or(syn::parse_quote! { where });
    let bounded_types = collect_memento_bounds(ir, env);
    where_clause.predicates.extend(bounded_types);
    (!where_clause.predicates.is_empty()).then_some(where_clause)
}

fn collect_memento_bounds(ir: &EnumIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds_for_enum(ir, env, &env.recallable_trait)
}

fn build_memento_variant(
    variant: &VariantIr<'_>,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    let name = variant.name;
    let mut fields = variant
        .kept_fields()
        .map(|(_, field)| build_memento_field(field, recallable_trait, generic_type_params))
        .peekable();
    let non_empty = fields.peek().is_some();
    match variant.shape {
        VariantShape::Named if non_empty => quote! { #name { #(#fields),* } },
        VariantShape::Unnamed if non_empty => quote! { #name(#(#fields),*) },
        _ => quote! { #name },
    }
}

fn build_memento_field(
    field: &FieldIr<'_>,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    build_memento_field_tokens(field, recallable_trait, generic_type_params)
}

fn build_marker_variant(marker_ty: &TokenStream2) -> TokenStream2 {
    let serde_attr = SERDE_ENABLED.then_some(quote! { #[serde(skip)] });

    quote! {
        #[doc(hidden)]
        #serde_attr
        __RecallableMarker(#marker_ty)
    }
}
