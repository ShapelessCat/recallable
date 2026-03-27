use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::internal::enums::{EnumIr, collect_recall_like_bounds_for_enum};
use crate::context::internal::shared::CodegenEnv;

#[must_use]
pub(crate) fn gen_enum_recall_impl(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let recall_trait = &env.recall_trait;
    let impl_generics = ir.impl_generics();
    let enum_type = ir.enum_type();
    let where_clause =
        ir.extend_where_clause(collect_recall_like_bounds_for_enum(ir, env, recall_trait));

    quote! {
        impl #impl_generics #recall_trait
            for #enum_type
        #where_clause {
            #[inline]
            fn recall(&mut self, memento: Self::Memento) {
                *self = Self::__recallable_restore_from_memento(memento);
            }
        }
    }
}
