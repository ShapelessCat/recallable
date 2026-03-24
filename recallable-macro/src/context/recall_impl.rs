use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::MacroContext;
use crate::context::{CodegenEnv, FieldIr, FieldMember, FieldStrategy, RecallPath, StructIr};

impl<'a> MacroContext<'a> {
    // ============================================================
    // impl<T, ...> Recall for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_recall_trait_impl(&self) -> TokenStream2 {
        let recall_trait = &self.recall_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let extra_trait_bounds = self.build_trait_bounds(recall_trait);
        let where_clause = self.extend_where_clause(&extra_trait_bounds);

        let input_struct_name = self.struct_name;

        let recall_param_name = if self.field_actions.is_empty() {
            quote! { _memento }
        } else {
            quote! { memento }
        };

        let recall_method_body = self.generate_recall_method_body();
        quote! {
            impl #impl_generics #recall_trait
                for #input_struct_name #type_generics
            #where_clause {
                #[inline(always)]
                fn recall(&mut self, #recall_param_name: Self::Memento) {
                    #recall_method_body
                }
            }
        }
    }

    fn generate_recall_method_body(&self) -> TokenStream2 {
        let statements = self
            .field_actions
            .iter()
            .enumerate()
            .map(|(recall_index, action)| {
                action.build_update_statement(&self.recall_trait, recall_index)
            });

        quote! { #(#statements)* }
    }
}

#[allow(dead_code)]
pub(crate) fn gen_recall_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let recall_trait = &env.recall_trait;
    let (impl_generics, type_generics, _) = ir.generics.split_for_impl();
    let extra_bounds = ir.recallable_bounds(recall_trait);
    let where_clause = ir.extend_where_clause(&extra_bounds);
    let struct_name = ir.name;

    let memento_fields: Vec<_> = ir.memento_fields().collect();

    let recall_param_name = if memento_fields.is_empty() {
        quote! { _memento }
    } else {
        quote! { memento }
    };

    let statements = memento_fields
        .iter()
        .map(|field| build_recall_statement_ir(field, recall_trait));

    quote! {
        impl #impl_generics #recall_trait
            for #struct_name #type_generics
        #where_clause {
            #[inline(always)]
            fn recall(&mut self, #recall_param_name: Self::Memento) {
                #(#statements)*
            }
        }
    }
}

#[allow(dead_code)]
fn build_recall_statement_ir(field: &FieldIr, recall_trait: &TokenStream2) -> TokenStream2 {
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
