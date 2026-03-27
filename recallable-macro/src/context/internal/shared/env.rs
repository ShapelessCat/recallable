use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::util::crate_path;

#[derive(Debug)]
pub(crate) struct CodegenEnv {
    pub(crate) recallable_trait: TokenStream2,
    pub(crate) recall_trait: TokenStream2,
}

impl CodegenEnv {
    #[must_use]
    pub(crate) fn resolve() -> Self {
        let crate_path = crate_path();
        Self {
            recallable_trait: quote! { #crate_path::Recallable },
            recall_trait: quote! { #crate_path::Recall },
        }
    }
}
