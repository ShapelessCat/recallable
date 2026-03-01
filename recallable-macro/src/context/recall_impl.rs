use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::MacroContext;

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
