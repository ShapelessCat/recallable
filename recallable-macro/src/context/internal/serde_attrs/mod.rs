pub(crate) mod merge;
pub(crate) mod parse;
pub(crate) mod types;

use syn::Fields;
use syn::spanned::Spanned;

use crate::context::internal::shared::fields::has_recallable_skip_attr;

pub(crate) use merge::MergeMode;
pub(crate) use types::{SerdeEnumAttrs, SerdeStructAttrs};

use merge::merge_field_attrs;
use parse::{parse_recallable_serde_attrs, parse_serde_attrs};

/// Run the serde attribute analysis pass over a struct's fields.
pub(crate) fn analyze_struct_serde_attrs(
    fields: &Fields,
    mode: MergeMode,
) -> syn::Result<SerdeStructAttrs> {
    let mut result = Vec::with_capacity(fields.len());
    for field in fields.iter() {
        let recallable = parse_recallable_serde_attrs(field)?;
        let serde = parse_serde_attrs(field)?;

        // Reject rename/alias on skipped fields
        if has_recallable_skip_attr(field)
            && (recallable.rename.is_some() || !recallable.aliases.is_empty())
        {
            return Err(syn::Error::new_spanned(
                field,
                "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
            ));
        }

        let merged = merge_field_attrs(
            recallable,
            serde,
            mode,
            field.ident.as_ref()
                .map(|i| i.span())
                .unwrap_or_else(|| field.ty.span()),
        )?;
        result.push(merged);
    }
    Ok(SerdeStructAttrs { fields: result })
}

/// Run the serde attribute analysis pass over an enum's variants.
pub(crate) fn analyze_enum_serde_attrs(
    data: &syn::DataEnum,
    mode: MergeMode,
) -> syn::Result<SerdeEnumAttrs> {
    let mut variants = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        let mut fields = Vec::with_capacity(variant.fields.len());
        for field in variant.fields.iter() {
            let recallable = parse_recallable_serde_attrs(field)?;
            let serde = parse_serde_attrs(field)?;

            if has_recallable_skip_attr(field)
                && (recallable.rename.is_some() || !recallable.aliases.is_empty())
            {
                return Err(syn::Error::new_spanned(
                    field,
                    "`rename` and `alias` cannot be used on a `#[recallable(skip)]` field",
                ));
            }

            let merged = merge_field_attrs(
                recallable,
                serde,
                mode,
                field.ident.as_ref()
                    .map(|i| i.span())
                    .unwrap_or_else(|| field.ty.span()),
            )?;
            fields.push(merged);
        }
        variants.push(fields);
    }
    Ok(SerdeEnumAttrs { variants })
}
