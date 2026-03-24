use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::{CodegenEnv, FieldIr, FieldMember, FieldStrategy, RecallPath, StructIr};

pub(crate) fn gen_recall_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let recall_trait = &env.recall_trait;
    let impl_generics = ir.impl_generics();
    let where_clause = {
        let extra_bounds = ir.recallable_bounds(recall_trait);
        ir.extend_where_clause(&extra_bounds)
    };
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

fn build_recall_method(ir: &StructIr, recall_trait: &TokenStream2) -> TokenStream2 {
    let memento_fields: Vec<_> = ir.memento_fields().collect();

    let recall_param_name = if memento_fields.is_empty() {
        quote! { _memento }
    } else {
        quote! { memento }
    };

    let statements = memento_fields
        .iter()
        .map(|field| build_recall_statement(field, recall_trait));

    quote! {
        #[inline(always)]
        fn recall(&mut self, #recall_param_name: Self::Memento) {
            #(#statements)*
        }
    }
}

fn build_recall_statement(field: &FieldIr, recall_trait: &TokenStream2) -> TokenStream2 {
    let member = &field.member;
    let memento_index = field
        .memento_index
        .expect("memento_fields() guarantees memento_index is Some");

    let memento_member = match member {
        FieldMember::Named(name) => quote! { #name },
        FieldMember::Unnamed(_) => {
            let idx = syn::Index::from(memento_index);
            quote! { #idx }
        }
    };

    match &field.strategy {
        FieldStrategy::StoreAsSelf => {
            quote! { self.#member = memento.#memento_member; }
        }
        FieldStrategy::StoreAsMemento(RecallPath::WholeType) => {
            quote! { #recall_trait::recall(&mut self.#member, memento.#memento_member); }
        }
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    }
}
