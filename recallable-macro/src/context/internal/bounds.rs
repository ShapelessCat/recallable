use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use super::SERDE_ENABLED;
use super::ir::{CodegenEnv, StructIr};

#[derive(Debug)]
pub(crate) struct MementoTraitSpec {
    common_traits: Vec<TokenStream2>,
    serde_derive_trait: Option<TokenStream2>,
    serde_nested_bound_trait: Option<TokenStream2>,
}

impl MementoTraitSpec {
    fn new(serde_enabled: bool) -> Self {
        Self {
            common_traits: vec![
                quote!(::core::clone::Clone),
                quote!(::core::fmt::Debug),
                quote!(::core::cmp::PartialEq),
            ],
            serde_derive_trait: serde_enabled.then_some(quote!(::serde::Deserialize)),
            serde_nested_bound_trait: serde_enabled
                .then_some(quote!(::serde::de::DeserializeOwned)),
        }
    }

    pub(crate) fn current() -> Self {
        Self::new(SERDE_ENABLED)
    }

    pub(crate) fn derive_attr(&self) -> TokenStream2 {
        let mut derive_traits = self.common_traits.clone();
        if let Some(serde_derive_trait) = &self.serde_derive_trait {
            derive_traits.push(serde_derive_trait.clone());
        }
        quote! { #[derive(#(#derive_traits),*)] }
    }

    fn common_bound_tokens(&self) -> TokenStream2 {
        let common_traits = &self.common_traits;
        quote! { #(#common_traits)+* }
    }

    fn serde_nested_bound(&self) -> Option<TokenStream2> {
        self.serde_nested_bound_trait.clone()
    }
}

pub(crate) fn collect_shared_memento_bounds(
    ir: &StructIr,
    env: &CodegenEnv,
) -> Vec<WherePredicate> {
    collect_shared_memento_bounds_with_spec(ir, env, &MementoTraitSpec::current())
}

fn collect_shared_memento_bounds_with_spec(
    ir: &StructIr,
    env: &CodegenEnv,
    memento_trait_spec: &MementoTraitSpec,
) -> Vec<WherePredicate> {
    let recallable_trait = &env.recallable_trait;
    let memento_trait_bounds = memento_trait_spec.common_bound_tokens();

    let mut bounds = ir.recallable_memento_bounds(&memento_trait_bounds);
    bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
    if let Some(deserialize_owned) = memento_trait_spec.serde_nested_bound() {
        bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
    }

    bounds
}

pub(crate) fn collect_recall_like_bounds(
    ir: &StructIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
) -> Vec<WherePredicate> {
    collect_recall_like_bounds_with_spec(ir, env, direct_bound, &MementoTraitSpec::current())
}

fn collect_recall_like_bounds_with_spec(
    ir: &StructIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
    memento_trait_spec: &MementoTraitSpec,
) -> Vec<WherePredicate> {
    let shared_memento_bounds =
        collect_shared_memento_bounds_with_spec(ir, env, memento_trait_spec);
    let shared_param_bound_count = ir.recallable_params().count();

    let mut bounds = ir.recallable_bounds(direct_bound);
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

#[cfg(test)]
mod tests {
    use quote::{ToTokens, quote};
    use syn::parse_quote;

    use super::{
        CodegenEnv, MementoTraitSpec, StructIr, collect_recall_like_bounds_with_spec,
        collect_shared_memento_bounds_with_spec,
    };

    #[test]
    fn shared_bound_helpers_preserve_predicate_order() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<T, U, V> {
                #[recallable]
                current: T,
                #[recallable]
                history: Option<U>,
                extra: V,
            }
        };
        let ir = StructIr::analyze(&input).unwrap();
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
        };

        let shared_bounds: Vec<_> =
            collect_shared_memento_bounds_with_spec(&ir, &env, &MementoTraitSpec::new(true))
                .into_iter()
                .map(|predicate| predicate.to_token_stream().to_string())
                .collect();
        assert_eq!(
            shared_bounds,
            vec![
                quote!(T::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::serde::de::DeserializeOwned)
                    .to_string(),
            ]
        );

        let recall_like_bounds: Vec<_> = collect_recall_like_bounds_with_spec(
            &ir,
            &env,
            &env.recall_trait,
            &MementoTraitSpec::new(true),
        )
        .into_iter()
        .map(|predicate| predicate.to_token_stream().to_string())
        .collect();
        assert_eq!(
            recall_like_bounds,
            vec![
                quote!(T: ::recallable::Recall).to_string(),
                quote!(T::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(Option<U>: ::recallable::Recall).to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::serde::de::DeserializeOwned)
                    .to_string(),
            ]
        );
    }

    #[test]
    fn whole_type_bound_helpers_deduplicate_repeated_types() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<T> {
                #[recallable]
                current: T,
                #[recallable]
                first: Wrapper<T>,
                #[recallable]
                second: Wrapper<T>,
            }
        };
        let ir = StructIr::analyze(&input).unwrap();
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
        };

        let whole_type_bounds: Vec<_> = ir
            .whole_type_bounds(&env.recallable_trait)
            .into_iter()
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        assert_eq!(
            whole_type_bounds,
            vec![quote!(Wrapper<T>: ::recallable::Recallable).to_string()]
        );

        let memento_trait_bounds = MementoTraitSpec::new(true).common_bound_tokens();
        let whole_type_memento_bounds: Vec<_> = ir
            .whole_type_memento_bounds(&env.recallable_trait, &memento_trait_bounds)
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        assert_eq!(
            whole_type_memento_bounds,
            vec![
                quote!(<Wrapper<T> as ::recallable::Recallable>::Memento:
                    ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq)
                .to_string()
            ]
        );

        let whole_type_from_bounds: Vec<_> = ir
            .whole_type_from_bounds(&env.recallable_trait)
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        let wrapper_memento_from_bound: syn::WherePredicate = parse_quote! {
            <Wrapper<T> as ::recallable::Recallable>::Memento:
                ::core::convert::From<Wrapper<T>>
        };
        assert_eq!(
            whole_type_from_bounds,
            vec![
                quote!(Wrapper<T>: ::recallable::Recallable).to_string(),
                wrapper_memento_from_bound.to_token_stream().to_string(),
            ]
        );

        let recall_like_bounds: Vec<_> = collect_recall_like_bounds_with_spec(
            &ir,
            &env,
            &env.recallable_trait,
            &MementoTraitSpec::new(true),
        )
        .into_iter()
        .map(|predicate| predicate.to_token_stream().to_string())
        .collect();
        assert_eq!(
            recall_like_bounds,
            vec![
                quote!(T: ::recallable::Recallable).to_string(),
                quote!(T::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(Wrapper<T>: ::recallable::Recallable).to_string(),
                quote!(<Wrapper<T> as ::recallable::Recallable>::Memento:
                    ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Wrapper<T> as ::recallable::Recallable>::Memento:
                    ::serde::de::DeserializeOwned)
                .to_string(),
            ]
        );
    }

    #[test]
    fn memento_trait_spec_formats_derives_for_serde_modes() {
        let serde_derives = MementoTraitSpec::new(true).derive_attr().to_string();
        assert!(serde_derives.contains(":: core :: clone :: Clone"));
        assert!(serde_derives.contains(":: core :: fmt :: Debug"));
        assert!(serde_derives.contains(":: core :: cmp :: PartialEq"));
        assert!(serde_derives.contains(":: serde :: Deserialize"));

        let no_serde_derives = MementoTraitSpec::new(false).derive_attr().to_string();
        assert!(no_serde_derives.contains(":: core :: clone :: Clone"));
        assert!(no_serde_derives.contains(":: core :: fmt :: Debug"));
        assert!(no_serde_derives.contains(":: core :: cmp :: PartialEq"));
        assert!(!no_serde_derives.contains(":: serde :: Deserialize"));
    }
}
