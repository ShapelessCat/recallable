pub(crate) mod merge;
pub(crate) mod parse;
pub(crate) mod types;

use syn::Fields;
use syn::spanned::Spanned;

use crate::context::SERDE_ENABLED;
use crate::context::internal::shared::fields::has_recallable_skip_attr;

pub(crate) use types::{SerdeEnumAttrs, SerdeFieldAttrs, SerdeStructAttrs};

use merge::merge_field_attrs;
use parse::{parse_recallable_serde_attrs, parse_serde_attrs};

/// Run the serde attribute analysis pass over a struct's fields.
pub(crate) fn analyze_struct_serde_attrs(fields: &Fields) -> syn::Result<SerdeStructAttrs> {
    let mut result = Vec::with_capacity(fields.len());
    let mut errors: Option<syn::Error> = None;

    for field in fields.iter() {
        // Parse failures bail immediately — syntax is broken, can't continue
        let recallable = parse_recallable_serde_attrs(field)?;
        let serde = parse_serde_attrs(field)?;

        let field_span = field
            .ident
            .as_ref()
            .map(|i| i.span())
            .unwrap_or_else(|| field.ty.span());

        let mut field_ok = true;

        // Validation: no-serde feature check
        if !SERDE_ENABLED && (recallable.rename.is_some() || !recallable.aliases.is_empty()) {
            let err = syn::Error::new(
                field_span,
                "`rename` and `alias` in `#[recallable(...)]` require the `serde` feature",
            );
            match &mut errors {
                Some(e) => e.combine(err),
                None => errors = Some(err),
            }
            field_ok = false;
        }

        // Validation: skip + rename/alias conflict
        if has_recallable_skip_attr(field)
            && (recallable.rename.is_some() || !recallable.aliases.is_empty())
        {
            let err = syn::Error::new_spanned(
                field,
                "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
            );
            match &mut errors {
                Some(e) => e.combine(err),
                None => errors = Some(err),
            }
            field_ok = false;
        }

        // Merge — only if no validation errors for this field
        if field_ok {
            match merge_field_attrs(recallable, serde, field_span) {
                Ok(merged) => result.push(merged),
                Err(err) => {
                    match &mut errors {
                        Some(e) => e.combine(err),
                        None => errors = Some(err),
                    }
                    result.push(SerdeFieldAttrs::default());
                }
            }
        } else {
            result.push(SerdeFieldAttrs::default());
        }
    }

    if let Some(e) = errors {
        Err(e)
    } else {
        Ok(SerdeStructAttrs { fields: result })
    }
}

/// Run the serde attribute analysis pass over an enum's variants.
pub(crate) fn analyze_enum_serde_attrs(data: &syn::DataEnum) -> syn::Result<SerdeEnumAttrs> {
    let mut variants = Vec::with_capacity(data.variants.len());
    let mut errors: Option<syn::Error> = None;

    for variant in &data.variants {
        let mut fields = Vec::with_capacity(variant.fields.len());
        for field in variant.fields.iter() {
            let recallable = parse_recallable_serde_attrs(field)?;
            let serde = parse_serde_attrs(field)?;

            let field_span = field
                .ident
                .as_ref()
                .map(|i| i.span())
                .unwrap_or_else(|| field.ty.span());

            let mut field_ok = true;

            if !SERDE_ENABLED && (recallable.rename.is_some() || !recallable.aliases.is_empty()) {
                let err = syn::Error::new(
                    field_span,
                    "`rename` and `alias` in `#[recallable(...)]` require the `serde` feature",
                );
                match &mut errors {
                    Some(e) => e.combine(err),
                    None => errors = Some(err),
                }
                field_ok = false;
            }

            if has_recallable_skip_attr(field)
                && (recallable.rename.is_some() || !recallable.aliases.is_empty())
            {
                let err = syn::Error::new_spanned(
                    field,
                    "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
                );
                match &mut errors {
                    Some(e) => e.combine(err),
                    None => errors = Some(err),
                }
                field_ok = false;
            }

            if field_ok {
                match merge_field_attrs(recallable, serde, field_span) {
                    Ok(merged) => fields.push(merged),
                    Err(err) => {
                        match &mut errors {
                            Some(e) => e.combine(err),
                            None => errors = Some(err),
                        }
                        fields.push(SerdeFieldAttrs::default());
                    }
                }
            } else {
                fields.push(SerdeFieldAttrs::default());
            }
        }
        variants.push(fields);
    }

    if let Some(e) = errors {
        Err(e)
    } else {
        Ok(SerdeEnumAttrs { variants })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{Fields, parse_quote};

    fn struct_fields(input: &syn::ItemStruct) -> &Fields {
        &input.fields
    }

    #[cfg(not(feature = "serde"))]
    #[test]
    fn accumulates_no_serde_errors_across_fields() {
        let input: syn::ItemStruct = parse_quote! {
            struct Example {
                #[recallable(rename = "a")]
                first: i32,
                #[recallable(alias = "b")]
                second: i32,
            }
        };
        let err = analyze_struct_serde_attrs(struct_fields(&input)).unwrap_err();
        let msg = err.to_string();
        // Both fields should be reported
        assert!(
            msg.contains("serde"),
            "expected serde feature error, got: {msg}"
        );
        // syn::Error with combine produces multiple error messages joined
        let errors: Vec<_> = err.into_iter().collect();
        assert_eq!(
            errors.len(),
            2,
            "expected 2 accumulated errors, got {}",
            errors.len()
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn accumulates_skip_rename_errors_across_fields() {
        let input: syn::ItemStruct = parse_quote! {
            struct Example {
                #[recallable(skip, rename = "a")]
                first: i32,
                #[recallable(skip, alias = "b")]
                second: i32,
            }
        };
        let err = analyze_struct_serde_attrs(struct_fields(&input)).unwrap_err();
        let errors: Vec<_> = err.into_iter().collect();
        assert_eq!(
            errors.len(),
            2,
            "expected 2 accumulated errors, got {}",
            errors.len()
        );
    }
}
