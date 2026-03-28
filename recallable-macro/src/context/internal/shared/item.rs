use syn::DeriveInput;

use crate::context::internal::{enums::EnumIr, structs::StructIr};

use super::util::is_recallable_attr;

#[derive(Debug)]
pub(crate) enum ItemIr<'a> {
    Struct(StructIr<'a>),
    Enum(EnumIr<'a>),
}

pub(crate) fn has_skip_memento_default_derives(input: &DeriveInput) -> syn::Result<bool> {
    let mut skip_memento_default_derives = false;
    for attr in input.attrs.iter().filter(|a| is_recallable_attr(a)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip_memento_default_derives") {
                skip_memento_default_derives = true;
                Ok(())
            } else if meta.path.is_ident("skip") {
                Err(meta.error("`skip` is a field-level attribute, not a struct-level attribute"))
            } else {
                Err(meta.error("unrecognized `recallable` parameter"))
            }
        })?;
    }
    Ok(skip_memento_default_derives)
}

impl<'a> ItemIr<'a> {
    pub(crate) fn analyze(input: &'a DeriveInput) -> syn::Result<Self> {
        match &input.data {
            syn::Data::Struct(_) => Ok(Self::Struct(StructIr::analyze(input)?)),
            syn::Data::Enum(_) => Ok(Self::Enum(EnumIr::analyze(input)?)),
            _ => Err(syn::Error::new_spanned(
                input,
                "This derive macro can only be applied to structs or enums",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::has_skip_memento_default_derives;

    #[test]
    fn struct_level_unknown_recallable_parameter_is_rejected() {
        let input: syn::DeriveInput = parse_quote! {
            #[recallable(unknown)]
            struct Example {
                value: u32,
            }
        };

        let error = has_skip_memento_default_derives(&input).unwrap_err();

        assert_eq!(error.to_string(), "unrecognized `recallable` parameter");
    }
}
