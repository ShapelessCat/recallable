use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    DeriveInput, Fields, Generics, Ident, ImplGenerics, Type, Visibility, WhereClause,
    WherePredicate,
};

use crate::context::SERDE_ENABLED;
use crate::context::internal::shared::bounds::MementoTraitSpec;
use crate::context::internal::shared::fields::{FieldIr, FieldStrategy, collect_field_irs};
use crate::context::internal::shared::generics::{
    GenericParamLookup, GenericParamPlan, collect_marker_param_indices, is_generic_type_param,
    marker_component, plan_memento_generics,
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
        let (usage, field_irs) = collect_field_irs(fields, &struct_lifetimes, &generic_lookup)?;
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

    #[must_use]
    pub(crate) fn memento_decl_generics(&self) -> TokenStream2 {
        let mut params = self
            .generic_params
            .iter()
            .filter(|plan| plan.is_retained())
            .map(GenericParamPlan::decl_param)
            .peekable();

        if params.peek().is_none() {
            quote! {}
        } else {
            quote! { <#(#params),*> }
        }
    }

    pub(crate) const fn memento_where_clause(&self) -> Option<&WhereClause> {
        self.memento_where_clause.as_ref()
    }

    #[must_use]
    pub(crate) fn generated_memento_shape(&self) -> StructShape {
        if self.shape == StructShape::Unit && self.has_synthetic_marker() {
            StructShape::Named
        } else {
            self.shape
        }
    }

    #[must_use]
    pub(crate) const fn has_synthetic_marker(&self) -> bool {
        !self.marker_param_indices.is_empty()
    }

    #[must_use]
    pub(crate) fn synthetic_marker_type(&self) -> Option<TokenStream2> {
        if self.marker_param_indices.is_empty() {
            return None;
        }

        let components = self
            .marker_param_indices
            .iter()
            .map(|&index| marker_component(self.generic_params[index].param));

        Some(quote! {
            ::core::marker::PhantomData<(#(#components,)*)>
        })
    }

    pub(crate) fn recallable_params(&self) -> impl Iterator<Item = &Ident> {
        self.generic_params
            .iter()
            .filter_map(GenericParamPlan::recallable_ident)
    }

    #[must_use]
    pub(crate) fn memento_type(&self) -> TokenStream2 {
        let name = &self.memento_name;
        let mut args = self
            .generic_params
            .iter()
            .filter(|plan| plan.is_retained())
            .map(GenericParamPlan::type_arg)
            .peekable();

        if args.peek().is_none() {
            quote! { #name }
        } else {
            quote! { #name<#(#args),*> }
        }
    }

    pub(crate) fn memento_fields(&self) -> impl Iterator<Item = &FieldIr<'a>> {
        self.fields.iter().filter(|field| !field.strategy.is_skip())
    }

    pub(crate) fn recallable_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.recallable_params()
            .map(|ty| syn::parse_quote! { #ty: #bound })
            .collect()
    }

    pub(crate) fn recallable_memento_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.recallable_params()
            .map(|ty| syn::parse_quote! { #ty::Memento: #bound })
            .collect()
    }

    fn whole_type_bound_targets(&self) -> impl Iterator<Item = &Type> {
        let mut seen = HashSet::new();

        self.fields
            .iter()
            .filter_map(move |field| match field.strategy {
                FieldStrategy::StoreAsMemento
                    if !is_generic_type_param(field.ty, &self.generic_type_param_idents)
                        && seen.insert(field.ty) =>
                {
                    Some(field.ty)
                }
                _ => None,
            })
    }

    #[must_use]
    pub(crate) fn whole_type_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.whole_type_bound_targets()
            .map(|ty| syn::parse_quote! { #ty: #bound })
            .collect()
    }

    pub(crate) fn whole_type_memento_bounds(
        &self,
        recallable_trait: &TokenStream2,
        bound: &TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> {
        self.whole_type_bound_targets()
            .map(move |ty| syn::parse_quote! { <#ty as #recallable_trait>::Memento: #bound })
    }

    pub(crate) fn whole_type_from_bounds(
        &self,
        recallable_trait: &TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> {
        self.whole_type_bound_targets().flat_map(move |ty| {
            [
                syn::parse_quote! { #ty: #recallable_trait },
                syn::parse_quote! { <#ty as #recallable_trait>::Memento: ::core::convert::From<#ty> },
            ]
        })
    }

    pub(crate) fn extend_where_clause(
        &self,
        extra: impl IntoIterator<Item = WherePredicate>,
    ) -> Option<WhereClause> {
        let mut where_clause = self.generics.where_clause.clone();
        let mut extra_iter = extra.into_iter().peekable();
        if extra_iter.peek().is_none() {
            where_clause
        } else {
            where_clause
                .get_or_insert(syn::parse_quote! { where })
                .predicates
                .extend(extra_iter);

            where_clause
        }
    }
}
