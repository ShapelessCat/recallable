use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Fields, Generics, Ident, ImplGenerics, Visibility, WhereClause};

use crate::context::SERDE_ENABLED;
use crate::context::internal::shared::bounds::MementoTraitSpec;
use crate::context::internal::shared::codegen::CodegenItemIr;
use crate::context::internal::shared::fields::{FieldIr, collect_field_irs};
use crate::context::internal::shared::generics::{
    GenericParamLookup, GenericParamPlan, collect_marker_param_indices, plan_memento_generics,
};
use crate::context::internal::shared::item::has_skip_memento_default_derives;
use crate::context::internal::shared::lifetime::{
    collect_struct_lifetimes, validate_no_borrowed_fields,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructShape {
    Named,
    Unnamed,
    Unit,
}

impl StructShape {
    const fn from_fields(fields: &Fields) -> Self {
        match fields {
            Fields::Named(_) => Self::Named,
            Fields::Unnamed(_) => Self::Unnamed,
            Fields::Unit => Self::Unit,
        }
    }
}

#[derive(Debug)]
pub(crate) struct StructIr<'a> {
    name: &'a Ident,
    visibility: &'a Visibility,
    generics: &'a Generics,
    shape: StructShape,
    fields: Vec<FieldIr<'a>>,
    memento_name: Ident,
    generic_type_param_idents: HashSet<&'a Ident>,
    generic_params: Vec<GenericParamPlan<'a>>,
    memento_where_clause: Option<WhereClause>,
    marker_param_indices: Vec<usize>,
    skip_memento_default_derives: bool,
}

fn extract_struct_fields(input: &DeriveInput) -> syn::Result<&Fields> {
    if let syn::Data::Struct(data) = &input.data {
        Ok(&data.fields)
    } else {
        Err(syn::Error::new_spanned(
            input,
            "This derive macro can only be applied to structs",
        ))
    }
}

impl<'a> StructIr<'a> {
    pub(crate) fn analyze(input: &'a DeriveInput) -> syn::Result<Self> {
        let fields = extract_struct_fields(input)?;
        let struct_lifetimes = collect_struct_lifetimes(&input.generics);
        validate_no_borrowed_fields(fields, &struct_lifetimes)?;

        let shape = StructShape::from_fields(fields);
        let memento_name = quote::format_ident!("{}Memento", input.ident);
        let generic_lookup = GenericParamLookup::new(&input.generics);
        let generic_type_param_idents = input
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect();
        let (usage, field_irs) = collect_field_irs(fields, &generic_lookup)?;
        let (generic_params, memento_where_clause) =
            plan_memento_generics(&input.generics, usage, &generic_lookup);
        let marker_param_indices =
            collect_marker_param_indices(&field_irs, &generic_params, &generic_lookup);
        let skip_memento_default_derives = has_skip_memento_default_derives(input)?;

        Ok(Self {
            name: &input.ident,
            visibility: &input.vis,
            generics: &input.generics,
            shape,
            fields: field_irs,
            memento_name,
            generic_type_param_idents,
            generic_params,
            memento_where_clause,
            marker_param_indices,
            skip_memento_default_derives,
        })
    }

    pub(crate) fn struct_type(&self) -> TokenStream2 {
        let name = &self.name;
        let (_, type_generics, _) = self.generics.split_for_impl();
        quote! { #name #type_generics }
    }

    pub(crate) const fn memento_name(&self) -> &Ident {
        &self.memento_name
    }

    pub(crate) const fn visibility(&self) -> &'a Visibility {
        self.visibility
    }

    #[must_use]
    pub(crate) const fn shape(&self) -> StructShape {
        self.shape
    }

    pub(crate) fn impl_generics(&self) -> ImplGenerics<'_> {
        let (impl_generics, _, _) = self.generics.split_for_impl();
        impl_generics
    }

    pub(crate) const fn generic_type_param_idents(&self) -> &HashSet<&'a Ident> {
        &self.generic_type_param_idents
    }

    #[must_use]
    pub(crate) const fn memento_trait_spec(&self) -> MementoTraitSpec {
        MementoTraitSpec::new(SERDE_ENABLED, self.skip_memento_default_derives)
    }

    pub(crate) const fn memento_where_clause(&self) -> Option<&WhereClause> {
        self.memento_where_clause.as_ref()
    }

    #[must_use]
    pub(crate) fn generated_memento_shape(&self) -> StructShape {
        self.shape
    }

    #[must_use]
    pub(crate) const fn has_synthetic_marker(&self) -> bool {
        !self.marker_param_indices.is_empty()
    }

    pub(crate) fn memento_fields(&self) -> impl Iterator<Item = &FieldIr<'a>> {
        self.fields.iter().filter(|field| !field.strategy.is_skip())
    }
}

impl<'a> CodegenItemIr<'a> for StructIr<'a> {
    type Fields<'b>
        = std::slice::Iter<'b, FieldIr<'a>>
    where
        Self: 'b,
        'a: 'b;

    fn generics(&self) -> &'a Generics {
        self.generics
    }

    fn memento_name(&self) -> &Ident {
        &self.memento_name
    }

    fn generic_type_param_idents(&self) -> &HashSet<&'a Ident> {
        &self.generic_type_param_idents
    }

    fn generic_params(&self) -> &[GenericParamPlan<'a>] {
        &self.generic_params
    }

    fn marker_param_indices(&self) -> &[usize] {
        &self.marker_param_indices
    }

    fn all_fields(&self) -> Self::Fields<'_> {
        self.fields.iter()
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::{StructIr, StructShape};

    #[test]
    fn unit_structs_never_need_synthetic_markers() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<'a, T: From<U>, U, const N: usize>;
        };

        let ir = StructIr::analyze(&input).unwrap();

        assert_eq!(ir.shape(), StructShape::Unit);
        assert_eq!(ir.generated_memento_shape(), StructShape::Unit);
        assert!(!ir.has_synthetic_marker());
    }
}
