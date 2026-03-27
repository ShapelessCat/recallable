use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::{
    CodegenEnv, FieldIr, FieldMember, FieldStrategy, StructIr, collect_recall_like_bounds,
};

#[must_use]
pub(crate) fn gen_struct_recall_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let recall_trait = &env.recall_trait;
    let impl_generics = ir.impl_generics();
    let where_clause = build_recall_where_clause(ir, env);
    let struct_type = ir.struct_type();
    let recall_method = build_recall_method(ir, recall_trait);

    quote! {
        impl #impl_generics #recall_trait
            for #struct_type
        #where_clause {
            #recall_method
        }
    }
}

fn build_recall_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let extra_bounds = collect_recall_bounds(ir, env);
    ir.extend_where_clause(extra_bounds)
}

fn collect_recall_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds(ir, env, &env.recall_trait)
}

fn build_recall_method(ir: &StructIr, recall_trait: &TokenStream2) -> TokenStream2 {
    let mut memento_fields = ir.memento_fields().peekable();
    let recall_param_name = build_recall_param_name(memento_fields.peek().is_some());
    let statements = memento_fields.map(|field| build_recall_statement(field, recall_trait));

    quote! {
        #[inline]
        fn recall(&mut self, #recall_param_name: Self::Memento) {
            #(#statements)*
        }
    }
}

fn build_recall_param_name(has_memento_fields: bool) -> TokenStream2 {
    if has_memento_fields {
        quote! { memento }
    } else {
        quote! { _memento }
    }
}

fn build_recall_statement(field: &FieldIr, recall_trait: &TokenStream2) -> TokenStream2 {
    let member = &field.member;
    let memento_member = build_memento_member(field);

    match &field.strategy {
        FieldStrategy::StoreAsSelf => {
            quote! { self.#member = memento.#memento_member; }
        }
        FieldStrategy::StoreAsMemento => {
            quote! { #recall_trait::recall(&mut self.#member, memento.#memento_member); }
        }
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    }
}

fn build_memento_member(field: &FieldIr) -> TokenStream2 {
    let member = &field.member;
    match member {
        FieldMember::Named(name) => quote! { #name },
        FieldMember::Unnamed(_) => {
            let memento_index = field
                .memento_index
                .expect("memento_fields() guarantees memento_index is Some");
            let idx = syn::Index::from(memento_index);
            quote! { #idx }
        }
    }
}
