use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::internal::shared::{CodegenEnv, CodegenItemIr, FieldIr, build_from_value_expr};
use crate::context::internal::structs::{StructIr, StructShape, collect_shared_memento_bounds};

#[must_use]
pub(crate) fn gen_struct_from_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let struct_type = ir.struct_type();
    let memento_type = ir.memento_type();
    let where_clause = build_from_where_clause(ir, env);
    let from_method = build_from_method(ir);

    quote! {
        impl #impl_generics ::core::convert::From<#struct_type>
            for #memento_type
        #where_clause {
            #from_method
        }
    }
}

fn build_from_method(ir: &StructIr) -> TokenStream2 {
    let struct_type = ir.struct_type();
    let fn_body = build_from_body(ir);

    quote! {
        #[inline]
        fn from(value: #struct_type) -> Self {
            #fn_body
        }
    }
}

fn build_from_body(ir: &StructIr) -> TokenStream2 {
    match ir.shape() {
        StructShape::Named => build_named_from_body(ir),
        StructShape::Unnamed => build_unnamed_from_body(ir),
        StructShape::Unit => build_unit_from_body(ir),
    }
}

fn build_named_from_body(ir: &StructIr) -> TokenStream2 {
    let inits = ir
        .memento_fields()
        .map(build_named_from_field)
        .chain(ir.has_synthetic_marker().then(build_named_marker_init));

    quote! { Self { #(#inits),* } }
}

fn build_named_from_field(field: &FieldIr) -> TokenStream2 {
    let member = &field.member;
    let value = build_from_expr(field);
    quote! { #member: #value }
}

fn build_unnamed_from_body(ir: &StructIr) -> TokenStream2 {
    let values = ir
        .memento_fields()
        .map(build_from_expr)
        .chain(ir.has_synthetic_marker().then(build_marker_init));

    quote! { Self(#(#values),*) }
}

fn build_unit_from_body(ir: &StructIr) -> TokenStream2 {
    if ir.has_synthetic_marker() {
        quote! { Self { _recallable_marker: ::core::marker::PhantomData } }
    } else {
        quote! { Self }
    }
}

fn build_marker_init() -> TokenStream2 {
    quote! { ::core::marker::PhantomData }
}

fn build_named_marker_init() -> TokenStream2 {
    quote! { _recallable_marker: ::core::marker::PhantomData }
}

fn build_from_expr(field: &FieldIr) -> TokenStream2 {
    let member = &field.member;
    build_from_value_expr(quote! { value.#member }, field.strategy)
}

fn build_from_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let bounds = collect_from_bounds(ir, env);
    ir.extend_where_clause(bounds)
}

fn collect_from_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    let recallable_trait = &env.recallable_trait;
    let mut bounds: Vec<_> = ir
        .recallable_params()
        .flat_map(|ty| -> [WherePredicate; 2] {
            [
                syn::parse_quote! { #ty: #recallable_trait },
                syn::parse_quote! { #ty::Memento: ::core::convert::From<#ty> },
            ]
        })
        .collect();
    bounds.extend(collect_shared_memento_bounds(ir, env));
    bounds.extend(ir.whole_type_from_bounds(recallable_trait));
    bounds
}
