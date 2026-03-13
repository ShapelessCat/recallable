use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Fields;

use crate::{IS_SERDE_ENABLED, context::MacroContext};

impl<'a> MacroContext<'a> {
    // ============================================================
    // #[derive(Debug, ::serde::Deserialize)]
    // struct InputTypeMemento<T, ...> ...
    // ============================================================

    pub(crate) fn build_memento_struct(&self) -> TokenStream2 {
        let derives = if IS_SERDE_ENABLED {
            quote! { #[derive(::core::fmt::Debug, ::serde::Deserialize)] }
        } else {
            quote! { #[derive(::core::fmt::Debug)] }
        };

        let memento_struct_type = &self.memento_struct_type;
        let bounded_types = self.build_trait_bounds(&self.recallable_trait);
        let where_clause = if bounded_types.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounded_types),* }
        };
        let recall_fields = self.field_actions.iter().map(|action| action.build_field());
        let body = match &self.fields {
            Fields::Named(_) => quote! { #where_clause { #(#recall_fields),* } },
            Fields::Unnamed(_) => quote! { ( #(#recall_fields),* ) #where_clause; },
            Fields::Unit => quote! {;},
        };
        quote! {
            #derives
            pub struct #memento_struct_type #body
        }
    }
}
