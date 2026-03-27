use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::{CodegenEnv, EnumIr, ItemIr, StructIr, collect_recall_like_bounds, collect_recall_like_bounds_for_enum};

#[must_use]
pub(crate) fn gen_recallable_impl(ir: &ItemIr, env: &CodegenEnv) -> TokenStream2 {
    match ir {
        ItemIr::Struct(ir) => gen_struct_recallable_impl(ir, env),
        ItemIr::Enum(ir) => gen_enum_recallable_impl(ir, env),
    }
}

fn gen_struct_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
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
    ir.extend_where_clause(extra_bounds)
}

fn collect_recallable_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds(ir, env, &env.recallable_trait)
}

fn gen_enum_recallable_impl(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let enum_type = ir.enum_type();
    let where_clause = build_enum_recallable_where_clause(ir, env);
    let memento_type = ir.memento_type();

    quote! {
        impl #impl_generics #recallable_trait
            for #enum_type
        #where_clause {
            type Memento = #memento_type;
        }
    }
}

fn build_enum_recallable_where_clause(
    ir: &EnumIr,
    env: &CodegenEnv,
) -> Option<syn::WhereClause> {
    let extra_bounds = collect_enum_recallable_bounds(ir, env);
    ir.extend_where_clause(extra_bounds)
}

fn collect_enum_recallable_bounds(ir: &EnumIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds_for_enum(ir, env, &env.recallable_trait)
}
