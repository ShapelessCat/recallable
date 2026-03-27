use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

#[derive(Debug)]
pub(crate) struct MementoTraitSpec {
    serde_enabled: bool,
    derive_off: bool,
}

impl MementoTraitSpec {
    pub(crate) const fn new(serde_enabled: bool, derive_off: bool) -> Self {
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

    pub(crate) fn common_bound_tokens(&self) -> TokenStream2 {
        if self.has_common_traits() {
            quote! { ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq }
        } else {
            quote! {}
        }
    }

    pub(crate) fn serde_nested_bound(&self) -> Option<TokenStream2> {
        self.serde_enabled
            .then_some(quote!(::serde::de::DeserializeOwned))
    }

    const fn has_common_traits(&self) -> bool {
        !self.derive_off
    }
}

#[cfg(test)]
mod tests {
    use super::MementoTraitSpec;

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
