use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, WherePredicate, parse_quote};

use crate::context::MacroContext;
use crate::context::{CodegenEnv, FieldIr, FieldStrategy, RecallPath, StructIr, StructShape};

impl<'a> MacroContext<'a> {
    // ======================================================================
    // impl<T, ...> From<OriginalStruct<T, ...>> for OriginalStructMemento<...>
    // ======================================================================

    pub(crate) fn build_from_trait_impl(&self) -> TokenStream2 {
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_where_clause_for_from_impl();

        let input_struct_name = self.struct_name;
        let memento_struct_type = &self.memento_struct_type;
        let from_method_body = self.build_from_method_body();

        quote! {
            impl #impl_generics ::core::convert::From<#input_struct_name #type_generics>
                for #memento_struct_type
            #where_clause {
                #[inline(always)]
                fn from(value: #input_struct_name #type_generics) -> Self {
                    #from_method_body
                }
            }
        }
    }

    fn build_from_method_body(&self) -> TokenStream2 {
        match &self.fields {
            Fields::Named(_) => {
                let field_initializers = self.field_actions.iter().map(|action| {
                    let member = &action.member;
                    let value = action.build_initializer_expr();
                    quote! { #member: #value }
                });
                quote! { Self { #(#field_initializers),* } }
            }
            Fields::Unnamed(_) => {
                let field_values = self
                    .field_actions
                    .iter()
                    .map(|action| action.build_initializer_expr());
                quote! { Self(#(#field_values),*) }
            }
            Fields::Unit => {
                debug_assert!(self.field_actions.is_empty());
                quote! { Self }
            }
        }
    }

    fn build_where_clause_for_from_impl(&self) -> Option<syn::WhereClause> {
        let recallable_trait = &self.recallable_trait;
        let trait_bounds: Vec<WherePredicate> = self
            .iter_recallable_type_params()
            .flat_map(|ty| {
                [
                    parse_quote! { #ty: #recallable_trait },
                    parse_quote! { #ty::Memento: ::core::convert::From<#ty> },
                ]
            })
            .collect();
        self.extend_where_clause(&trait_bounds)
    }
}

#[allow(dead_code)]
pub(crate) fn gen_from_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let (impl_generics, type_generics, _) = ir.generics.split_for_impl();
    let where_clause = build_from_where_clause(ir, env);
    let struct_name = ir.name;
    let memento_type = ir.memento_type();
    let body = build_from_body_ir(ir);

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

#[allow(dead_code)]
fn build_from_body_ir(ir: &StructIr) -> TokenStream2 {
    match ir.shape {
        StructShape::Named => {
            let inits = ir.memento_fields().map(|field| {
                let member = &field.member;
                let value = build_from_expr_ir(field);
                quote! { #member: #value }
            });
            quote! { Self { #(#inits),* } }
        }
        StructShape::Unnamed => {
            let values = ir.memento_fields().map(build_from_expr_ir);
            quote! { Self(#(#values),*) }
        }
        StructShape::Unit => {
            quote! { Self }
        }
    }
}

#[allow(dead_code)]
fn build_from_expr_ir(field: &FieldIr) -> TokenStream2 {
    let member = &field.member;
    match &field.strategy {
        FieldStrategy::StoreAsSelf => quote! { value.#member },
        FieldStrategy::StoreAsMemento(RecallPath::WholeType) => {
            quote! { ::core::convert::From::from(value.#member) }
        }
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    }
}

#[allow(dead_code)]
fn build_from_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let recallable_trait = &env.recallable_trait;
    let bounds: Vec<syn::WherePredicate> = ir
        .recallable_params()
        .flat_map(|ty| -> [syn::WherePredicate; 2] {
            [
                syn::parse_quote! { #ty: #recallable_trait },
                syn::parse_quote! { #ty::Memento: ::core::convert::From<#ty> },
            ]
        })
        .collect();
    ir.extend_where_clause(&bounds)
}
