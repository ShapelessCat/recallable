use std::collections::HashSet;

use syn::{Data, DataStruct, DeriveInput, Field, Fields, Index, Meta, PathArguments, Type};

use super::generics::{
    BareTypeParam, GenericParamLookup, GenericUsage, collect_generic_dependencies_in_type,
};
use super::ir::{FieldIr, FieldMember, FieldStrategy};
use super::lifetime::{field_uses_struct_lifetime, is_phantom_data};
use super::util::is_recallable_attr;

/// Field-level behavior inferred from `#[recallable]` attributes during analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldBehavior {
    /// Keep the field in the memento with its original type.
    Keep,
    /// Store the field as an inner memento and recall this field recursively.
    Recall,
}

pub(super) fn extract_struct_fields(input: &DeriveInput) -> syn::Result<&Fields> {
    if let Data::Struct(DataStruct { fields, .. }) = &input.data {
        Ok(fields)
    } else {
        Err(syn::Error::new_spanned(
            input,
            "This derive macro can only be applied to structs",
        ))
    }
}

fn determine_field_behavior(field: &Field) -> syn::Result<Option<FieldBehavior>> {
    let mut saw_recall = false;
    let mut saw_skip = false;

    for attr in field.attrs.iter().filter(|attr| is_recallable_attr(attr)) {
        match &attr.meta {
            Meta::Path(_) => {
                saw_recall = true;
            }
            Meta::List(_) => attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    saw_skip = true;
                    Ok(())
                } else {
                    Err(meta.error("unrecognized `recallable` parameter"))
                }
            })?,
            Meta::NameValue(_) => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "unrecognized `recallable` parameter",
                ));
            }
        }
    }

    if saw_recall && saw_skip {
        return Err(syn::Error::new_spanned(
            field,
            "conflicting `recallable` attributes: choose exactly one of `#[recallable]` or `#[recallable(skip)]`",
        ));
    }

    Ok(match (saw_recall, saw_skip) {
        (true, false) => Some(FieldBehavior::Recall), // #[recallable]
        (false, true) => None,                        // #[recallable(skip)]
        (false, false) => Some(FieldBehavior::Keep),
        (true, true) => unreachable!("conflicting attributes handled above"),
    })
}

fn field_member(field: &Field, index: usize) -> FieldMember<'_> {
    if let Some(field_name) = field.ident.as_ref() {
        FieldMember::Named(field_name)
    } else {
        FieldMember::Unnamed(Index::from(index))
    }
}

fn classify_recallable_field_type(
    field_type: &Type,
    generic_lookup: &GenericParamLookup<'_>,
) -> syn::Result<Option<BareTypeParam>> {
    match field_type {
        Type::Path(type_path)
            if type_path.qself.is_none()
                && type_path.path.segments.len() == 1
                && matches!(type_path.path.segments[0].arguments, PathArguments::None) =>
        {
            let ident = &type_path.path.segments[0].ident;
            Ok(generic_lookup.type_param_index(ident).map(BareTypeParam))
        }
        Type::Path(_) => Ok(None),
        _ => Err(syn::Error::new_spanned(
            field_type,
            "Only path types are supported here",
        )),
    }
}

pub(super) fn collect_field_irs<'a>(
    fields: &'a Fields,
    struct_lifetimes: &HashSet<&'a syn::Ident>,
    generic_lookup: &GenericParamLookup<'a>,
) -> syn::Result<(GenericUsage, Vec<FieldIr<'a>>)> {
    let mut usage = GenericUsage::default();
    let mut field_irs = Vec::with_capacity(fields.len());
    let mut memento_counter: usize = 0;

    for (index, field) in fields.iter().enumerate() {
        let member = field_member(field, index);
        let ty = &field.ty;

        if is_phantom_data(ty) && field_uses_struct_lifetime(ty, struct_lifetimes) {
            field_irs.push(FieldIr {
                memento_index: None,
                member,
                ty,
                strategy: FieldStrategy::Skip,
            });
            continue;
        }

        let strategy = match determine_field_behavior(field)? {
            None => FieldStrategy::Skip,
            Some(FieldBehavior::Keep) => {
                usage
                    .retained
                    .extend(collect_generic_dependencies_in_type(ty, generic_lookup));
                FieldStrategy::StoreAsSelf
            }
            Some(FieldBehavior::Recall) => {
                usage
                    .retained
                    .extend(collect_generic_dependencies_in_type(ty, generic_lookup));
                if let Some(BareTypeParam(index)) =
                    classify_recallable_field_type(ty, generic_lookup)?
                {
                    usage.recallable_type_params.insert(index);
                }
                FieldStrategy::StoreAsMemento
            }
        };

        let memento_index = (!strategy.is_skip()).then_some(memento_counter);
        field_irs.push(FieldIr {
            memento_index,
            member,
            ty,
            strategy,
        });
        if memento_index.is_some() {
            memento_counter += 1;
        }
    }

    Ok((usage, field_irs))
}

pub(crate) fn has_recallable_skip_attr(field: &Field) -> bool {
    // Use determine_field_behavior for consistent validation.
    // In the attribute macro context, we intentionally ignore errors here
    // because the derive macros will report them with proper spans.
    matches!(determine_field_behavior(field), Ok(None))
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::{GenericParamLookup, classify_recallable_field_type};

    #[test]
    fn recallable_type_classifier_accepts_any_path_type() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<T> {
                #[recallable]
                value: Option<T>,
            }
        };
        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => unreachable!(),
        };
        let field = fields.iter().next().unwrap();
        let lookup = GenericParamLookup::new(&input.generics);

        assert!(matches!(
            classify_recallable_field_type(&field.ty, &lookup),
            Ok(None)
        ));
    }
}
