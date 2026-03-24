use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::Ident;

use crate::context::{
    CodegenEnv, FieldIr, FieldMember, FieldStrategy, RecallPath, StructIr, StructShape,
    is_generic_type_param,
};

pub(crate) fn gen_memento_struct(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let derives = if env.serde_enabled {
        quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq, ::serde::Deserialize)] }
    } else {
        quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq)] }
    };

    let memento_name = ir.memento_name();
    let memento_generics = ir.memento_decl_generics();
    let recallable_trait = &env.recallable_trait;
    let memento_trait_bounds = quote! {
        ::core::clone::Clone
            + ::core::fmt::Debug
            + ::core::cmp::PartialEq
    };
    let mut bounded_types = ir.recallable_bounds(recallable_trait);
    bounded_types.extend(ir.recallable_memento_bounds(&memento_trait_bounds));
    bounded_types.extend(ir.whole_type_bounds(recallable_trait));
    bounded_types.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
    if env.serde_enabled {
        let deserialize_owned = quote! { ::serde::de::DeserializeOwned };
        bounded_types.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
    }
    let mut where_clause = ir
        .memento_where_clause()
        .cloned()
        .unwrap_or_else(|| syn::parse_quote! { where });
    if bounded_types.is_empty() && where_clause.predicates.is_empty() {
        where_clause.predicates.clear();
    } else {
        where_clause.predicates.extend(bounded_types);
    }
    let where_clause = (!where_clause.predicates.is_empty()).then_some(where_clause);

    let generic_type_params: HashSet<&Ident> = ir.type_params().map(|p| &p.ident).collect();

    let mut fields: Vec<_> = ir
        .memento_fields()
        .map(|field| build_memento_field(field, recallable_trait, &generic_type_params))
        .collect();
    if let Some(marker_ty) = ir.synthetic_marker_type() {
        fields.push(build_marker_field(
            &marker_ty,
            ir.generated_memento_shape(),
            env,
        ));
    }

    let body = match ir.generated_memento_shape() {
        StructShape::Named => quote! { #where_clause { #(#fields),* } },
        StructShape::Unnamed => quote! { ( #(#fields),* ) #where_clause; },
        StructShape::Unit => quote! { #where_clause; },
    };

    quote! {
        #derives
        pub struct #memento_name #memento_generics #body
    }
}

fn build_marker_field(
    marker_ty: &TokenStream2,
    shape: StructShape,
    env: &CodegenEnv,
) -> TokenStream2 {
    let serde_attr = env
        .serde_enabled
        .then(|| quote! { #[serde(skip, default)] });

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
        FieldStrategy::StoreAsMemento(RecallPath::WholeType) => {
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
