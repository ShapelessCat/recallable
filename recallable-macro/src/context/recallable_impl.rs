use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::{CodegenEnv, StructIr};

pub(crate) fn gen_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let struct_type = ir.struct_type();
    let where_clause = build_recallable_where_clause(ir, env);
    let memento_type = ir.memento_type();

    quote! {
        impl #impl_generics #recallable_trait
            for #struct_type
        #where_clause {
            type Memento = #memento_type;
        }
    }
}

fn build_recallable_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let extra_bounds = collect_recallable_bounds(ir, env);
    ir.extend_where_clause(&extra_bounds)
}

fn collect_recallable_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    let recallable_trait = &env.recallable_trait;
    let memento_trait_bounds = env.memento_trait_bounds();

    let mut bounds = ir.recallable_bounds(recallable_trait);
    bounds.extend(ir.recallable_memento_bounds(&memento_trait_bounds));
    bounds.extend(ir.whole_type_bounds(recallable_trait));
    bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
    if let Some(deserialize_owned) = env.deserialize_owned_bound() {
        bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
    }

    bounds
}
