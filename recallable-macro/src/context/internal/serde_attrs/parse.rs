use syn::{Field, LitStr};

const RECALLABLE: &str = "recallable";
const SERDE: &str = "serde";

/// Parsed rename/alias values from a single attribute source.
#[derive(Debug, Default)]
pub(crate) struct RawFieldSerdeAttrs {
    pub(crate) rename: Option<LitStr>,
    pub(crate) aliases: Vec<LitStr>,
}

/// Extract rename/alias from `#[recallable(...)]` attributes on a field.
pub(crate) fn parse_recallable_serde_attrs(field: &Field) -> syn::Result<RawFieldSerdeAttrs> {
    let mut result = RawFieldSerdeAttrs::default();

    for attr in field.attrs.iter().filter(|a| a.path().is_ident(RECALLABLE)) {
        if let syn::Meta::List(_) = &attr.meta {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    if result.rename.is_some() {
                        return Err(meta.error("duplicate `rename` in `#[recallable(...)]`"));
                    }
                    result.rename = Some(lit);
                    Ok(())
                } else if meta.path.is_ident("alias") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    result.aliases.push(lit);
                    Ok(())
                } else {
                    // skip/other params handled by determine_field_behavior
                    Ok(())
                }
            })?;
        }
    }

    Ok(result)
}

/// Extract rename/alias from `#[serde(...)]` attributes on a field.
pub(crate) fn parse_serde_attrs(field: &Field) -> syn::Result<RawFieldSerdeAttrs> {
    let mut result = RawFieldSerdeAttrs::default();

    for attr in field.attrs.iter().filter(|a| a.path().is_ident(SERDE)) {
        if let syn::Meta::List(_) = &attr.meta {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    if result.rename.is_some() {
                        return Err(meta.error("duplicate `rename` in `#[serde(...)]`"));
                    }
                    result.rename = Some(lit);
                } else if meta.path.is_ident("alias") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    result.aliases.push(lit);
                }
                // ignore other serde attrs — not our concern
                Ok(())
            })?;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn make_field(tokens: proc_macro2::TokenStream) -> Field {
        let item: syn::ItemStruct = parse_quote! {
            struct S { #tokens }
        };
        match &item.fields {
            syn::Fields::Named(f) => f.named.first().unwrap().clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn recallable_rename_parsed() {
        let field = make_field(quote::quote! {
            #[recallable(rename = "wire")]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert_eq!(attrs.rename.unwrap().value(), "wire");
        assert!(attrs.aliases.is_empty());
    }

    #[test]
    fn recallable_alias_parsed() {
        let field = make_field(quote::quote! {
            #[recallable(alias = "old", alias = "legacy")]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert!(attrs.rename.is_none());
        assert_eq!(attrs.aliases.len(), 2);
        assert_eq!(attrs.aliases[0].value(), "old");
        assert_eq!(attrs.aliases[1].value(), "legacy");
    }

    #[test]
    fn recallable_rename_and_alias_combined() {
        let field = make_field(quote::quote! {
            #[recallable(rename = "new", alias = "alt")]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert_eq!(attrs.rename.unwrap().value(), "new");
        assert_eq!(attrs.aliases.len(), 1);
    }

    #[test]
    fn serde_rename_parsed() {
        let field = make_field(quote::quote! {
            #[serde(rename = "wire")]
            value: i32
        });
        let attrs = parse_serde_attrs(&field).unwrap();
        assert_eq!(attrs.rename.unwrap().value(), "wire");
    }

    #[test]
    fn serde_alias_parsed() {
        let field = make_field(quote::quote! {
            #[serde(alias = "old")]
            value: i32
        });
        let attrs = parse_serde_attrs(&field).unwrap();
        assert_eq!(attrs.aliases.len(), 1);
        assert_eq!(attrs.aliases[0].value(), "old");
    }

    #[test]
    fn no_attrs_returns_empty() {
        let field = make_field(quote::quote! { value: i32 });
        let recallable = parse_recallable_serde_attrs(&field).unwrap();
        let serde = parse_serde_attrs(&field).unwrap();
        assert!(recallable.rename.is_none());
        assert!(recallable.aliases.is_empty());
        assert!(serde.rename.is_none());
        assert!(serde.aliases.is_empty());
    }

    #[test]
    fn recallable_skip_field_is_ignored() {
        let field = make_field(quote::quote! {
            #[recallable(skip)]
            value: i32
        });
        let attrs = parse_recallable_serde_attrs(&field).unwrap();
        assert!(attrs.rename.is_none());
        assert!(attrs.aliases.is_empty());
    }
}
