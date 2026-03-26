use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{
    DeriveInput, Fields, Generics, Ident, ImplGenerics, Index, Type, Visibility, WhereClause,
    WherePredicate,
};

use crate::context::SERDE_ENABLED;

use super::bounds::MementoTraitSpec;

use super::fields::{collect_field_irs, extract_struct_fields};
use super::generics::{
    GenericParamPlan, collect_marker_param_indices, is_generic_type_param, marker_component,
    plan_memento_generics,
};
use super::lifetime::{collect_struct_lifetimes, validate_no_borrowed_fields};
use super::util::{crate_path, is_recallable_attr};

/// The structural shape of the source struct and generated memento.
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

/// How a field is represented in the generated memento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldStrategy {
    /// Field excluded from memento entirely.
    Skip,
    /// Field copied as-is into memento (type unchanged).
    StoreAsSelf,
    /// Field stored as its memento type, recalled recursively.
    StoreAsMemento,
}

impl FieldStrategy {
    pub(super) const fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }
}

#[derive(Debug)]
pub(crate) struct CodegenEnv {
    /// Fully qualified path to the `Recallable` trait.
    pub(crate) recallable_trait: TokenStream2,
    /// Fully qualified path to the `Recall` trait.
    pub(crate) recall_trait: TokenStream2,
}

impl CodegenEnv {
    pub(crate) fn resolve() -> Self {
        let crate_path = crate_path();
        Self {
            recallable_trait: quote! { #crate_path::Recallable },
            recall_trait: quote! { #crate_path::Recall },
        }
    }
}

#[derive(Debug)]
pub(crate) struct FieldIr<'a> {
    pub(crate) memento_index: Option<usize>,
    pub(crate) member: FieldMember<'a>,
    pub(crate) ty: &'a Type,
    pub(crate) strategy: FieldStrategy,
}

/// How a field member is referenced in generated tokens.
#[derive(Debug, Clone)]
pub(crate) enum FieldMember<'a> {
    /// Access by named field, such as `value`.
    Named(&'a Ident),
    /// Access by tuple-field index, such as `.0`.
    Unnamed(Index),
}

impl<'a> ToTokens for FieldMember<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            FieldMember::Named(ident) => ident.to_tokens(tokens),
            FieldMember::Unnamed(index) => index.to_tokens(tokens),
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
    memento_derive_off: bool,
}

fn has_memento_derive_off(input: &DeriveInput) -> syn::Result<bool> {
    let mut found = false;
    for attr in input.attrs.iter().filter(|a| is_recallable_attr(a)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("memento_derive_off") {
                found = true;
                Ok(())
            } else if meta.path.is_ident("skip") {
                Err(meta.error("`skip` is a field-level attribute, not a struct-level attribute"))
            } else {
                Err(meta.error("unrecognized `recallable` parameter"))
            }
        })?;
    }
    Ok(found)
}

impl<'a> StructIr<'a> {
    pub(crate) fn analyze(input: &'a DeriveInput) -> syn::Result<Self> {
        let fields = extract_struct_fields(input)?;
        let struct_lifetimes = collect_struct_lifetimes(&input.generics);
        validate_no_borrowed_fields(fields, &struct_lifetimes)?;

        let shape = StructShape::from_fields(fields);
        let memento_name = quote::format_ident!("{}Memento", input.ident);
        let generic_lookup = super::generics::GenericParamLookup::new(&input.generics);
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
        let memento_derive_off = has_memento_derive_off(input)?;

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
            memento_derive_off,
        })
    }

    pub(crate) fn struct_type(&self) -> TokenStream2 {
        let name = &self.name;
        let (_, type_generics, _) = self.generics.split_for_impl();
        quote! { #name #type_generics }
    }

    pub(crate) fn memento_name(&self) -> &Ident {
        &self.memento_name
    }

    pub(crate) fn visibility(&self) -> &'a Visibility {
        self.visibility
    }

    pub(crate) fn shape(&self) -> StructShape {
        self.shape
    }

    pub(crate) fn impl_generics(&self) -> ImplGenerics<'_> {
        let (impl_generics, _, _) = self.generics.split_for_impl();
        impl_generics
    }

    pub(crate) fn generic_type_param_idents(&self) -> &HashSet<&'a Ident> {
        &self.generic_type_param_idents
    }

    pub(crate) fn memento_trait_spec(&self) -> MementoTraitSpec {
        MementoTraitSpec::new(SERDE_ENABLED, self.memento_derive_off)
    }

    pub(crate) fn memento_decl_generics(&self) -> TokenStream2 {
        let params = self
            .generic_params
            .iter()
            .filter(|plan| plan.is_retained())
            .map(GenericParamPlan::decl_param)
            .collect::<Vec<_>>();

        if params.is_empty() {
            quote! {}
        } else {
            quote! { <#(#params),*> }
        }
    }

    pub(crate) fn memento_where_clause(&self) -> Option<&WhereClause> {
        self.memento_where_clause.as_ref()
    }

    pub(crate) fn generated_memento_shape(&self) -> StructShape {
        if self.shape == StructShape::Unit && self.has_synthetic_marker() {
            StructShape::Named
        } else {
            self.shape
        }
    }

    pub(crate) fn has_synthetic_marker(&self) -> bool {
        !self.marker_param_indices.is_empty()
    }

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

    pub(crate) fn memento_type(&self) -> TokenStream2 {
        let name = &self.memento_name;
        let args: Vec<_> = self
            .generic_params
            .iter()
            .filter(|plan| plan.is_retained())
            .map(GenericParamPlan::type_arg)
            .collect();

        if args.is_empty() {
            quote! { #name }
        } else {
            quote! { #name<#(#args),*> }
        }
    }

    pub(crate) fn memento_fields(&self) -> impl Iterator<Item = &FieldIr<'a>> {
        self.fields.iter().filter(|field| !field.strategy.is_skip())
    }

    pub(super) fn recallable_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.recallable_params()
            .map(|ty| syn::parse_quote! { #ty: #bound })
            .collect()
    }

    pub(super) fn recallable_memento_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.recallable_params()
            .map(|ty| syn::parse_quote! { #ty::Memento: #bound })
            .collect()
    }

    fn whole_type_bound_targets(&self) -> Vec<&Type> {
        let mut seen = HashSet::new();

        self.fields
            .iter()
            .filter_map(|field| match field.strategy {
                FieldStrategy::StoreAsMemento
                    if !is_generic_type_param(field.ty, &self.generic_type_param_idents)
                        && seen.insert(field.ty) =>
                {
                    Some(field.ty)
                }
                _ => None,
            })
            .collect()
    }

    pub(super) fn whole_type_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.whole_type_bound_targets()
            .into_iter()
            .map(|ty| syn::parse_quote! { #ty: #bound })
            .collect()
    }

    pub(super) fn whole_type_memento_bounds(
        &self,
        recallable_trait: &TokenStream2,
        bound: &TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> {
        self.whole_type_bound_targets()
            .into_iter()
            .map(move |ty| syn::parse_quote! { <#ty as #recallable_trait>::Memento: #bound })
    }

    pub(crate) fn whole_type_from_bounds(
        &self,
        recallable_trait: &TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> {
        self.whole_type_bound_targets()
            .into_iter()
            .flat_map(move |ty| {
                [
                    syn::parse_quote! { #ty: #recallable_trait },
                    syn::parse_quote! { <#ty as #recallable_trait>::Memento: ::core::convert::From<#ty> },
                ]
            })
    }

    pub(crate) fn extend_where_clause(&self, extra: &[WherePredicate]) -> Option<WhereClause> {
        let mut where_clause = self.generics.where_clause.clone();
        if !extra.is_empty() {
            where_clause
                .get_or_insert_with(|| syn::parse_quote! { where })
                .predicates
                .extend(extra.iter().cloned());
        }
        where_clause
    }
}
