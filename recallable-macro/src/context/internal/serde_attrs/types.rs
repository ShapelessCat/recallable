use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

#[derive(Debug, Default)]
pub(crate) struct SerdeFieldAttrs {
    pub(crate) rename: Option<syn::LitStr>,
    pub(crate) aliases: Vec<syn::LitStr>,
}

/// Result of the serde attribute analysis pass for a struct.
#[derive(Debug)]
pub(crate) struct SerdeStructAttrs {
    /// Per-field attrs, indexed by field position.
    pub(crate) fields: Vec<SerdeFieldAttrs>,
}

/// Result of the serde attribute analysis pass for an enum.
#[derive(Debug)]
pub(crate) struct SerdeEnumAttrs {
    /// Per-variant, per-field attrs.
    pub(crate) variants: Vec<Vec<SerdeFieldAttrs>>,
}

impl SerdeFieldAttrs {
    #[must_use]
    pub(crate) fn to_memento_tokens(&self) -> TokenStream2 {
        let rename = self.rename.as_ref().map(|lit| {
            quote! { #[serde(rename = #lit)] }
        });
        let aliases = self.aliases.iter().map(|lit| {
            quote! { #[serde(alias = #lit)] }
        });
        quote! {
            #rename
            #(#aliases)*
        }
    }
}

/// Unified wrapper for passing serde attrs through the memento codegen dispatch.
#[derive(Debug)]
pub(crate) enum SerdeItemAttrs {
    Struct(SerdeStructAttrs),
    Enum(SerdeEnumAttrs),
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn empty_attrs_produce_no_tokens() {
        let attrs = SerdeFieldAttrs::default();
        assert!(attrs.to_memento_tokens().is_empty());
    }

    #[test]
    fn rename_produces_serde_rename_attr() {
        let attrs = SerdeFieldAttrs {
            rename: Some(parse_quote!("wire_name")),
            aliases: vec![],
        };
        let tokens = attrs.to_memento_tokens().to_string();
        assert!(tokens.contains("rename"));
        assert!(tokens.contains("wire_name"));
    }

    #[test]
    fn aliases_produce_serde_alias_attrs() {
        let attrs = SerdeFieldAttrs {
            rename: None,
            aliases: vec![parse_quote!("old"), parse_quote!("legacy")],
        };
        let tokens = attrs.to_memento_tokens().to_string();
        assert!(tokens.contains("alias"));
        assert!(tokens.contains("old"));
        assert!(tokens.contains("legacy"));
    }

    #[test]
    fn rename_and_aliases_combined() {
        let attrs = SerdeFieldAttrs {
            rename: Some(parse_quote!("new_name")),
            aliases: vec![parse_quote!("alt")],
        };
        let tokens = attrs.to_memento_tokens().to_string();
        assert!(tokens.contains("rename"));
        assert!(tokens.contains("new_name"));
        assert!(tokens.contains("alias"));
        assert!(tokens.contains("alt"));
    }
}
