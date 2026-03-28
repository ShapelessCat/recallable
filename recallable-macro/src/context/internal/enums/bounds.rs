use proc_macro2::TokenStream as TokenStream2;
use syn::WherePredicate;

use crate::context::internal::enums::EnumIr;
use crate::context::internal::shared::{CodegenEnv, MementoTraitSpec};

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
    let recallable_trait = &env.recallable_trait;
    let memento_trait_bounds = memento_trait_spec.common_bound_tokens();

    let mut bounds = ir
        .recallable_memento_bounds(&memento_trait_bounds)
        .collect::<Vec<_>>();
    bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
    if let Some(deserialize_owned) = memento_trait_spec.serde_nested_bound() {
        bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
    }

    bounds
}

fn collect_recall_like_bounds_with_spec_for_enum(
    ir: &EnumIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
    memento_trait_spec: &MementoTraitSpec,
) -> Vec<WherePredicate> {
    let shared_memento_bounds =
        collect_shared_memento_bounds_with_spec_for_enum(ir, env, memento_trait_spec);
    let shared_param_bound_count = ir.recallable_params().count();

    let mut bounds = ir.recallable_bounds(direct_bound).collect::<Vec<_>>();
    bounds.extend(
        shared_memento_bounds
            .iter()
            .take(shared_param_bound_count)
            .cloned(),
    );
    bounds.extend(ir.whole_type_bounds(direct_bound));
    bounds.extend(
        shared_memento_bounds
            .into_iter()
            .skip(shared_param_bound_count),
    );
    bounds
}
