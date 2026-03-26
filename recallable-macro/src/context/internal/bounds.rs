use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use super::ir::{CodegenEnv, StructIr};

/// Policy for which derives and nested bounds the generated memento should receive.
#[derive(Debug)]
pub(crate) struct MementoTraitSpec {
    serde_enabled: bool,
    derive_off: bool,
}

impl MementoTraitSpec {
    pub(super) const fn new(serde_enabled: bool, derive_off: bool) -> Self {
        Self {
            serde_enabled,
            derive_off,
        }
    }

    #[must_use]
    pub(crate) fn derive_attr(&self) -> TokenStream2 {
        match (self.has_common_traits(), self.serde_enabled) {
            (true, true) => {
                quote! {
                    #[derive(
                        ::core::clone::Clone,
                        ::core::fmt::Debug,
                        ::core::cmp::PartialEq,
                        ::serde::Deserialize
                    )]
                }
            }
            (true, false) => {
                quote! {
                    #[derive(
                        ::core::clone::Clone,
                        ::core::fmt::Debug,
                        ::core::cmp::PartialEq
                    )]
                }
            }
            (false, true) => quote! { #[derive(::serde::Deserialize)] },
            (false, false) => quote! {},
        }
    }

    fn common_bound_tokens(&self) -> TokenStream2 {
        if self.has_common_traits() {
            quote! { ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq }
        } else {
            quote! {}
        }
    }

    fn serde_nested_bound(&self) -> Option<TokenStream2> {
        self.serde_enabled
            .then(|| quote!(::serde::de::DeserializeOwned))
    }

    const fn has_common_traits(&self) -> bool {
        !self.derive_off
    }
}

#[must_use]
pub(crate) fn collect_shared_memento_bounds(
    ir: &StructIr,
    env: &CodegenEnv,
) -> Vec<WherePredicate> {
    collect_shared_memento_bounds_with_spec(ir, env, &ir.memento_trait_spec())
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

#[must_use]
pub(crate) fn collect_recall_like_bounds(
    ir: &StructIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
) -> Vec<WherePredicate> {
    collect_recall_like_bounds_with_spec(ir, env, direct_bound, &ir.memento_trait_spec())
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
            collect_shared_memento_bounds_with_spec(&ir, &env, &MementoTraitSpec::new(true, false))
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
            &MementoTraitSpec::new(true, false),
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

        let memento_trait_bounds = MementoTraitSpec::new(true, false).common_bound_tokens();
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
            &MementoTraitSpec::new(true, false),
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
        let serde_derives = MementoTraitSpec::new(true, false).derive_attr().to_string();
        assert!(serde_derives.contains(":: core :: clone :: Clone"));
        assert!(serde_derives.contains(":: core :: fmt :: Debug"));
        assert!(serde_derives.contains(":: core :: cmp :: PartialEq"));
        assert!(serde_derives.contains(":: serde :: Deserialize"));

        let no_serde_derives = MementoTraitSpec::new(false, false)
            .derive_attr()
            .to_string();
        assert!(no_serde_derives.contains(":: core :: clone :: Clone"));
        assert!(no_serde_derives.contains(":: core :: fmt :: Debug"));
        assert!(no_serde_derives.contains(":: core :: cmp :: PartialEq"));
        assert!(!no_serde_derives.contains(":: serde :: Deserialize"));
    }

    #[test]
    fn memento_trait_spec_derive_off_suppresses_common_traits() {
        let derive_off_serde = MementoTraitSpec::new(true, true).derive_attr().to_string();
        assert!(!derive_off_serde.contains("Clone"));
        assert!(!derive_off_serde.contains("Debug"));
        assert!(!derive_off_serde.contains("PartialEq"));
        assert!(derive_off_serde.contains(":: serde :: Deserialize"));

        let derive_off_no_serde = MementoTraitSpec::new(false, true).derive_attr().to_string();
        assert!(derive_off_no_serde.is_empty());
    }

    #[test]
    fn memento_trait_spec_derive_off_empties_common_bounds() {
        let spec = MementoTraitSpec::new(true, true);
        assert!(spec.common_bound_tokens().is_empty());
        assert!(spec.serde_nested_bound().is_some());
    }
}
