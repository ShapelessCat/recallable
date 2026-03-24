use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::{CodegenEnv, FieldIr, FieldStrategy, RecallPath, StructIr, StructShape};

pub(crate) fn gen_from_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
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
    let marker_init = ir
        .has_synthetic_marker()
        .then(|| quote! { ::core::marker::PhantomData });
    let fn_body = match ir.shape {
        StructShape::Named => {
            let mut inits: Vec<_> = ir
                .memento_fields()
                .map(|field| {
                    let member = &field.member;
                    let value = build_from_expr(field);
                    quote! { #member: #value }
                })
                .collect();
            if marker_init.is_some() {
                inits.push(quote! { _recallable_marker: ::core::marker::PhantomData });
            }
            quote! { Self { #(#inits),* } }
        }
        StructShape::Unnamed => {
            let mut values: Vec<_> = ir.memento_fields().map(build_from_expr).collect();
            if let Some(marker_init) = marker_init {
                values.push(marker_init);
            }
            quote! { Self(#(#values),*) }
        }
        StructShape::Unit => {
            if marker_init.is_some() {
                quote! { Self { _recallable_marker: ::core::marker::PhantomData } }
            } else {
                quote! { Self }
            }
        }
    };

    quote! {
        #[inline(always)]
        fn from(value: #struct_type) -> Self {
            #fn_body
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
    let memento_trait_bounds = quote! {
        ::core::clone::Clone
            + ::core::fmt::Debug
            + ::core::cmp::PartialEq
    };
    let mut bounds: Vec<WherePredicate> = ir
        .recallable_params()
        .flat_map(|ty| -> [WherePredicate; 2] {
            [
                syn::parse_quote! { #ty: #recallable_trait },
                syn::parse_quote! { #ty::Memento: ::core::convert::From<#ty> },
            ]
        })
        .collect();
    bounds.extend(ir.recallable_memento_bounds(&memento_trait_bounds));
    bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
    if env.serde_enabled {
        let deserialize_owned = quote! { ::serde::de::DeserializeOwned };
        bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
    }
    bounds.extend(ir.whole_type_from_bounds(recallable_trait));
    ir.extend_where_clause(&bounds)
}
