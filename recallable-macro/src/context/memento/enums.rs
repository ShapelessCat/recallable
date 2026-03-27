use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Ident, WhereClause, WherePredicate};

use crate::context::SERDE_ENABLED;
use crate::context::internal::enums::{
    EnumIr, VariantIr, VariantShape, collect_recall_like_bounds_for_enum,
};
use crate::context::internal::shared::{
    CodegenEnv, FieldIr, FieldMember, FieldStrategy, is_generic_type_param,
};

#[must_use]
pub(crate) fn gen_memento_enum(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let derives = ir.memento_trait_spec().derive_attr();
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

    if where_clause.predicates.is_empty() {
        None
    } else {
        Some(where_clause)
    }
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
    let fields: Vec<_> = variant
        .fields
        .iter()
        .filter(|field| !field.strategy.is_skip())
        .map(|field| build_memento_field(field, recallable_trait, generic_type_params))
        .collect();

    let shape = if fields.is_empty() {
        VariantShape::Unit
    } else {
        variant.shape
    };

    match shape {
        VariantShape::Named => quote! { #name { #(#fields),* } },
        VariantShape::Unnamed => quote! { #name(#(#fields),*) },
        VariantShape::Unit => quote! { #name },
    }
}

fn build_memento_field(
    field: &FieldIr<'_>,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    let ty = field.ty;
    let field_ty = match &field.strategy {
        FieldStrategy::StoreAsMemento => {
            if is_generic_type_param(ty, generic_type_params) {
                quote! { #ty::Memento }
            } else {
                quote! { <#ty as #recallable_trait>::Memento }
            }
        }
        FieldStrategy::StoreAsSelf => quote! { #ty },
        FieldStrategy::Skip => unreachable!("filtered above"),
    };

    match &field.member {
        FieldMember::Named(name) => quote! { #name: #field_ty },
        FieldMember::Unnamed(_) => quote! { #field_ty },
    }
}

fn build_marker_variant(marker_ty: &TokenStream2) -> TokenStream2 {
    let serde_attr = SERDE_ENABLED.then_some(quote! { #[serde(skip)] });

    quote! {
        #[doc(hidden)]
        #serde_attr
        __RecallableMarker(#marker_ty)
    }
}
