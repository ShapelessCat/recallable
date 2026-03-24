use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::{CodegenEnv, FieldIr, FieldStrategy, RecallPath, StructIr, StructShape};

pub(crate) fn gen_from_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let (impl_generics, type_generics, _) = ir.generics.split_for_impl();
    let where_clause = build_from_where_clause(ir, env);
    let struct_name = ir.name;
    let memento_type = ir.memento_type();
    let body = build_from_body(ir);

    quote! {
        impl #impl_generics ::core::convert::From<#struct_name #type_generics>
            for #memento_type
        #where_clause {
            #[inline(always)]
            fn from(value: #struct_name #type_generics) -> Self {
                #body
            }
        }
    }
}

fn build_from_body(ir: &StructIr) -> TokenStream2 {
    match ir.shape {
        StructShape::Named => {
            let inits = ir.memento_fields().map(|field| {
                let member = &field.member;
                let value = build_from_expr(field);
                quote! { #member: #value }
            });
            quote! { Self { #(#inits),* } }
        }
        StructShape::Unnamed => {
            let values = ir.memento_fields().map(build_from_expr);
            quote! { Self(#(#values),*) }
        }
        StructShape::Unit => {
            quote! { Self }
        }
    }
}

fn build_from_expr(field: &FieldIr) -> TokenStream2 {
    let member = &field.member;
    match &field.strategy {
        FieldStrategy::StoreAsSelf => quote! { value.#member },
        FieldStrategy::StoreAsMemento(RecallPath::WholeType) => {
            quote! { ::core::convert::From::from(value.#member) }
        }
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    }
}

fn build_from_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let recallable_trait = &env.recallable_trait;
    let bounds: Vec<WherePredicate> = ir
        .recallable_params()
        .flat_map(|ty| -> [WherePredicate; 2] {
            [
                syn::parse_quote! { #ty: #recallable_trait },
                syn::parse_quote! { #ty::Memento: ::core::convert::From<#ty> },
            ]
        })
        .collect();
    ir.extend_where_clause(&bounds)
}
