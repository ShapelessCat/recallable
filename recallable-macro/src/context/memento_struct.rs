use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{Ident, WhereClause, WherePredicate};

use crate::context::{
    CodegenEnv, FieldIr, FieldMember, FieldStrategy, StructIr, StructShape,
    collect_recall_like_bounds, is_generic_type_param,
};

pub(crate) fn gen_memento_struct(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let derives = build_memento_derives(env);
    let memento_name = ir.memento_name();
    let memento_generics = ir.memento_decl_generics();
    let body = build_memento_body(ir, env);

    quote! {
        #derives
        pub struct #memento_name #memento_generics #body
    }
}

fn build_memento_derives(env: &CodegenEnv) -> TokenStream2 {
    if env.serde_enabled {
        quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq, ::serde::Deserialize)] }
    } else {
        quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq)] }
    }
}

fn build_memento_body(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let shape = ir.generated_memento_shape();
    let where_clause = build_memento_where_clause(ir, env);
    let fields = collect_memento_fields(ir, env, shape);

    match shape {
        StructShape::Named => quote! { #where_clause { #(#fields),* } },
        StructShape::Unnamed => quote! { ( #(#fields),* ) #where_clause; },
        StructShape::Unit => quote! { #where_clause; },
    }
}

fn build_memento_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<WhereClause> {
    let mut where_clause = ir
        .memento_where_clause()
        .cloned()
        .unwrap_or_else(|| syn::parse_quote! { where });
    let bounded_types = collect_memento_bounds(ir, env);

    if bounded_types.is_empty() && where_clause.predicates.is_empty() {
        return None;
    }

    where_clause.predicates.extend(bounded_types);
    Some(where_clause)
}

fn collect_memento_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds(ir, env, &env.recallable_trait)
}

fn collect_memento_fields(
    ir: &StructIr,
    env: &CodegenEnv,
    shape: StructShape,
) -> Vec<TokenStream2> {
    let recallable_trait = &env.recallable_trait;
    let mut fields: Vec<_> = ir
        .memento_fields()
        .map(|field| build_memento_field(field, recallable_trait, ir.generic_type_param_idents()))
        .collect();

    if let Some(marker_ty) = ir.synthetic_marker_type() {
        fields.push(build_marker_field(&marker_ty, shape, env));
    }

    fields
}

fn build_marker_field(
    marker_ty: &TokenStream2,
    shape: StructShape,
    env: &CodegenEnv,
) -> TokenStream2 {
    let serde_attr = env
        .serde_enabled
        .then_some(quote! { #[serde(skip, default)] });

    match shape {
        StructShape::Named => quote! {
            #serde_attr
            _recallable_marker: #marker_ty
        },
        StructShape::Unnamed => quote! {
            #serde_attr
            #marker_ty
        },
        StructShape::Unit => unreachable!("unit mementos with synthetic markers become named"),
    }
}

fn build_memento_field(
    field: &FieldIr,
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
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    };
    match &field.member {
        FieldMember::Named(name) => quote! { #name: #field_ty },
        FieldMember::Unnamed(_) => quote! { #field_ty },
    }
}
