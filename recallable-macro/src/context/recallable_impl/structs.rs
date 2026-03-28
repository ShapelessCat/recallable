use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::internal::shared::{CodegenEnv, CodegenItemIr};
use crate::context::internal::structs::{StructIr, collect_recall_like_bounds};

#[must_use]
pub(crate) fn gen_struct_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let struct_type = ir.struct_type();
    let where_clause = build_recallable_where_clause(ir, env);
    let memento_type = ir.memento_type();

    quote! {
        #[automatically_derived]
        impl #impl_generics #recallable_trait
            for #struct_type
        #where_clause {
            type Memento = #memento_type;
        }
    }
}

fn build_recallable_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let extra_bounds = collect_recallable_bounds(ir, env);
    ir.extend_where_clause(extra_bounds)
}

fn collect_recallable_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds(ir, env, &env.recallable_trait)
}
