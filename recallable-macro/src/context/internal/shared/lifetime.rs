use std::collections::HashSet;

use syn::visit::Visit;
use syn::{Fields, GenericParam, Generics, Ident, Type};

use super::fields::has_recallable_skip_attr;

pub(crate) fn validate_no_borrowed_fields(
    fields: &Fields,
    struct_lifetimes: &HashSet<&Ident>,
) -> syn::Result<()> {
    if struct_lifetimes.is_empty() {
        return Ok(());
    }

    let mut errors: Option<syn::Error> = None;

    for field in fields.iter() {
        if has_recallable_skip_attr(field) {
            continue;
        }
        if is_phantom_data(&field.ty) {
            continue;
        }
        if field_uses_struct_lifetime(&field.ty, struct_lifetimes) {
            let err =
                syn::Error::new_spanned(&field.ty, "Recall derives do not support borrowed fields");
            match &mut errors {
                Some(existing) => existing.combine(err),
                None => errors = Some(err),
            }
        }
    }

    match errors {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[must_use]
pub(crate) fn collect_struct_lifetimes(generics: &Generics) -> HashSet<&Ident> {
    generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Lifetime(lt) => Some(&lt.lifetime.ident),
            _ => None,
        })
        .collect()
}

struct LifetimeUsageChecker<'a> {
    struct_lifetimes: &'a HashSet<&'a Ident>,
    found: bool,
}

impl<'ast> Visit<'ast> for LifetimeUsageChecker<'_> {
    fn visit_lifetime(&mut self, lt: &'ast syn::Lifetime) {
        if self.struct_lifetimes.contains(&lt.ident) {
            self.found = true;
        }
    }
}

#[must_use]
pub(crate) fn is_phantom_data(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(p)
        if p.path.segments.last().is_some_and(|seg| seg.ident == "PhantomData")
    )
}

#[must_use]
pub(crate) fn field_uses_struct_lifetime(ty: &Type, struct_lifetimes: &HashSet<&Ident>) -> bool {
    let mut checker = LifetimeUsageChecker {
        struct_lifetimes,
        found: false,
    };
    checker.visit_type(ty);
    checker.found
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::{collect_struct_lifetimes, is_phantom_data, validate_no_borrowed_fields};

    #[test]
    fn phantom_data_detection_accepts_common_path_variants() {
        assert!(is_phantom_data(&parse_quote!(PhantomData<u8>)));
        assert!(is_phantom_data(&parse_quote!(marker::PhantomData<u8>)));
        assert!(is_phantom_data(&parse_quote!(
            core::marker::PhantomData<u8>
        )));
        assert!(is_phantom_data(&parse_quote!(
            ::core::marker::PhantomData<u8>
        )));
        assert!(is_phantom_data(&parse_quote!(std::marker::PhantomData<u8>)));
        assert!(is_phantom_data(&parse_quote!(
            ::std::marker::PhantomData<u8>
        )));
    }

    #[test]
    fn borrowed_field_validation_rejects_non_skipped_borrows() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<'a> {
                value: &'a str,
                #[recallable(skip)]
                skipped: &'a str,
                marker: ::core::marker::PhantomData<&'a ()>,
            }
        };
        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => unreachable!(),
        };
        let struct_lifetimes = collect_struct_lifetimes(&input.generics);
        let error = validate_no_borrowed_fields(fields, &struct_lifetimes).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Recall derives do not support borrowed fields")
        );
    }
}
