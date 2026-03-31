use super::parse::RawFieldSerdeAttrs;
use super::types::SerdeFieldAttrs;

/// Merge `#[recallable(...)]` and `#[serde(...)]` attrs for a single field.
/// Returns the merged `SerdeFieldAttrs` or a compile error on conflict.
pub(crate) fn merge_field_attrs(
    recallable: RawFieldSerdeAttrs,
    serde: RawFieldSerdeAttrs,
    field_span: proc_macro2::Span,
) -> syn::Result<SerdeFieldAttrs> {
    // Merge rename
    let rename = match (recallable.rename, serde.rename) {
        (Some(r), Some(s)) => {
            if r.value() != s.value() {
                return Err(syn::Error::new(
                    field_span,
                    format!(
                        "conflicting `rename` values: `#[serde(rename = \"{}\")]` and \
                         `#[recallable(rename = \"{}\")]` must match",
                        s.value(),
                        r.value(),
                    ),
                ));
            }
            Some(r)
        }
        (Some(r), None) => Some(r),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    };

    // Merge aliases: union and deduplicate by string value
    let mut seen = std::collections::BTreeSet::new();
    let mut aliases = Vec::new();
    for lit in recallable.aliases.into_iter().chain(serde.aliases) {
        if seen.insert(lit.value()) {
            aliases.push(lit);
        }
    }

    Ok(SerdeFieldAttrs { rename, aliases })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;
    use syn::LitStr;

    fn lit(s: &str) -> LitStr {
        LitStr::new(s, Span::call_site())
    }

    #[test]
    fn both_empty_produces_empty() {
        let result = merge_field_attrs(
            RawFieldSerdeAttrs::default(),
            RawFieldSerdeAttrs::default(),
            Span::call_site(),
        )
        .unwrap();
        assert!(result.rename.is_none() && result.aliases.is_empty());
    }

    #[test]
    fn recallable_rename_only() {
        let recallable = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result =
            merge_field_attrs(recallable, RawFieldSerdeAttrs::default(), Span::call_site())
                .unwrap();
        assert_eq!(result.rename.unwrap().value(), "x");
    }

    #[test]
    fn serde_rename_only_in_derive_mode() {
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result =
            merge_field_attrs(RawFieldSerdeAttrs::default(), serde, Span::call_site()).unwrap();
        assert_eq!(result.rename.unwrap().value(), "x");
    }

    #[test]
    fn matching_rename_values_merge() {
        let recallable = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let result = merge_field_attrs(recallable, serde, Span::call_site()).unwrap();
        assert_eq!(result.rename.unwrap().value(), "x");
    }

    #[test]
    fn conflicting_rename_values_rejected() {
        let recallable = RawFieldSerdeAttrs {
            rename: Some(lit("x")),
            aliases: vec![],
        };
        let serde = RawFieldSerdeAttrs {
            rename: Some(lit("y")),
            aliases: vec![],
        };
        let result = merge_field_attrs(recallable, serde, Span::call_site());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("conflicting"));
    }

    #[test]
    fn aliases_are_unioned_and_deduplicated() {
        let recallable = RawFieldSerdeAttrs {
            rename: None,
            aliases: vec![lit("a"), lit("b")],
        };
        let serde = RawFieldSerdeAttrs {
            rename: None,
            aliases: vec![lit("b"), lit("c")],
        };
        let result = merge_field_attrs(recallable, serde, Span::call_site()).unwrap();
        let values: Vec<String> = result.aliases.iter().map(|l| l.value()).collect();
        assert_eq!(values, vec!["a", "b", "c"]);
    }
}
