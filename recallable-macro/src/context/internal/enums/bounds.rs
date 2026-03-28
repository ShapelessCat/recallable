use proc_macro2::TokenStream as TokenStream2;
use syn::WherePredicate;

use crate::context::internal::enums::EnumIr;
use crate::context::internal::shared::{
    CodegenEnv, MementoTraitSpec, collect_recall_like_bounds as collect_shared_recall_like_bounds,
    collect_shared_memento_bounds as collect_common_memento_bounds,
};

#[must_use]
pub(crate) fn collect_shared_memento_bounds_for_enum(
    ir: &EnumIr,
    env: &CodegenEnv,
) -> Vec<WherePredicate> {
    collect_shared_memento_bounds_with_spec_for_enum(ir, env, &ir.memento_trait_spec())
}

#[must_use]
pub(crate) fn collect_recall_like_bounds_for_enum(
    ir: &EnumIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
) -> Vec<WherePredicate> {
    collect_recall_like_bounds_with_spec_for_enum(ir, env, direct_bound, &ir.memento_trait_spec())
}

fn collect_shared_memento_bounds_with_spec_for_enum(
    ir: &EnumIr,
    env: &CodegenEnv,
    memento_trait_spec: &MementoTraitSpec,
) -> Vec<WherePredicate> {
    collect_common_memento_bounds(ir, env, memento_trait_spec)
}

fn collect_recall_like_bounds_with_spec_for_enum(
    ir: &EnumIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
    memento_trait_spec: &MementoTraitSpec,
) -> Vec<WherePredicate> {
    collect_shared_recall_like_bounds(ir, env, direct_bound, memento_trait_spec)
}
