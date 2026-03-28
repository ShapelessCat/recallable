use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    DeriveInput, Generics, Ident, ImplGenerics, Type, Visibility, WhereClause, WherePredicate,
};

use crate::context::SERDE_ENABLED;
use crate::context::internal::shared::FieldMember;
use crate::context::internal::shared::bounds::MementoTraitSpec;
use crate::context::internal::shared::fields::{FieldIr, FieldStrategy, collect_field_irs};
use crate::context::internal::shared::generics::{
    GenericParamLookup, GenericParamPlan, collect_variant_marker_param_indices,
    is_generic_type_param, marker_component, plan_memento_generics,
};
use crate::context::internal::shared::item::has_skip_memento_default_derives;
use crate::context::internal::shared::lifetime::{
    collect_struct_lifetimes, validate_no_borrowed_fields,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariantShape {
    Named,
    Unnamed,
    Unit,
}

#[derive(Debug)]
pub(crate) struct VariantIr<'a> {
    pub(crate) name: &'a Ident,
    pub(crate) shape: VariantShape,
    pub(crate) fields: Vec<FieldIr<'a>>,
}

#[derive(Debug)]
pub(crate) struct EnumIr<'a> {
    name: &'a Ident,
    visibility: &'a Visibility,
    generics: &'a Generics,
    variants: Vec<VariantIr<'a>>,
    memento_name: Ident,
    generic_type_param_idents: HashSet<&'a Ident>,
    generic_params: Vec<GenericParamPlan<'a>>,
    memento_where_clause: Option<WhereClause>,
    marker_param_indices: Vec<usize>,
    skip_memento_default_derives: bool,
}

pub(crate) fn build_binding_ident(field: &FieldIr<'_>, index: usize) -> syn::Ident {
    match &field.member {
        FieldMember::Named(name) => (*name).clone(),
        FieldMember::Unnamed(_) => format_ident!("__recallable_field_{index}"),
    }
}

const ENUM_RECALL_MANUAL_ONLY_ERROR: &str = "enum `Recall` derive requires assignment-only variant fields; derive `Recallable` and \
     implement `Recall` or `TryRecall` manually";
const ENUM_MODEL_MANUAL_ONLY_ERROR: &str = "`#[recallable_model]` on enums requires assignment-only variants; complex enums should \
     derive `Recallable` and implement `Recall` or `TryRecall` manually";

fn extract_enum_variants(
    input: &DeriveInput,
) -> syn::Result<&syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>> {
    if let syn::Data::Enum(data) = &input.data {
        Ok(&data.variants)
    } else {
        Err(syn::Error::new_spanned(
            input,
            "This derive macro can only be applied to structs or enums",
        ))
    }
}

fn collect_variant_irs<'a>(
    variants: &'a syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    struct_lifetimes: &HashSet<&'a syn::Ident>,
    generic_lookup: &GenericParamLookup<'a>,
) -> syn::Result<(
    crate::context::internal::shared::generics::GenericUsage,
    Vec<VariantIr<'a>>,
)> {
    let mut usage = crate::context::internal::shared::generics::GenericUsage::default();
    let mut variant_irs = Vec::with_capacity(variants.len());

    for variant in variants {
        let (variant_usage, fields) =
            collect_field_irs(&variant.fields, struct_lifetimes, generic_lookup)?;
        usage.retained.extend(variant_usage.retained);
        usage
            .recallable_type_params
            .extend(variant_usage.recallable_type_params);

        let shape = match &variant.fields {
            syn::Fields::Named(_) => VariantShape::Named,
            syn::Fields::Unnamed(_) => VariantShape::Unnamed,
            syn::Fields::Unit => VariantShape::Unit,
        };

        variant_irs.push(VariantIr {
            name: &variant.ident,
            shape,
            fields,
        });
    }

    Ok((usage, variant_irs))
}

impl<'a> EnumIr<'a> {
    pub(crate) fn analyze(input: &'a DeriveInput) -> syn::Result<Self> {
        let variants = extract_enum_variants(input)?;
        let struct_lifetimes = collect_struct_lifetimes(&input.generics);
        for variant in variants {
            validate_no_borrowed_fields(&variant.fields, &struct_lifetimes)?;
        }

        let generic_lookup = GenericParamLookup::new(&input.generics);
        let generic_type_param_idents = input
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect();
        let (usage, variant_irs) =
            collect_variant_irs(variants, &struct_lifetimes, &generic_lookup)?;
        let (generic_params, memento_where_clause) =
            plan_memento_generics(&input.generics, usage, &generic_lookup);
        let marker_param_indices =
            collect_variant_marker_param_indices(&variant_irs, &generic_params, &generic_lookup);

        Ok(Self {
            name: &input.ident,
            visibility: &input.vis,
            generics: &input.generics,
            variants: variant_irs,
            memento_name: quote::format_ident!("{}Memento", input.ident),
            generic_type_param_idents,
            generic_params,
            memento_where_clause,
            marker_param_indices,
            skip_memento_default_derives: has_skip_memento_default_derives(input)?,
        })
    }

    pub(crate) const fn name(&self) -> &'a Ident {
        self.name
    }

    pub(crate) fn enum_type(&self) -> TokenStream2 {
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

    pub(crate) fn variants(&self) -> impl Iterator<Item = &VariantIr<'a>> {
        self.variants.iter()
    }

    fn manual_only_field(&self) -> Option<&FieldIr<'a>> {
        self.variants
            .iter()
            .flat_map(|variant| variant.fields.iter())
            .find(|field| !matches!(field.strategy, FieldStrategy::StoreAsSelf))
    }

    pub(crate) fn supports_derived_recall(&self) -> bool {
        self.manual_only_field().is_none()
    }

    pub(crate) fn ensure_recall_derive_allowed(&self) -> syn::Result<()> {
        if let Some(field) = self.manual_only_field() {
            return Err(syn::Error::new_spanned(
                field.source,
                ENUM_RECALL_MANUAL_ONLY_ERROR,
            ));
        }

        Ok(())
    }

    pub(crate) fn ensure_model_derive_allowed(&self) -> syn::Result<()> {
        if self.manual_only_field().is_some() {
            return Err(syn::Error::new_spanned(
                self.name,
                ENUM_MODEL_MANUAL_ONLY_ERROR,
            ));
        }

        Ok(())
    }

    pub(crate) fn recallable_bounds(
        &self,
        bound: &TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> {
        self.recallable_params()
            .map(move |ty| syn::parse_quote! { #ty: #bound })
    }

    pub(crate) fn recallable_memento_bounds(
        &self,
        bound: &TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> {
        self.recallable_params()
            .map(move |ty| syn::parse_quote! { #ty::Memento: #bound })
    }

    fn whole_type_bound_targets(&self) -> impl Iterator<Item = &Type> {
        let mut seen = HashSet::new();

        self.variants
            .iter()
            .flat_map(|variant| variant.fields.iter())
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

    pub(crate) fn whole_type_bounds<'b>(
        &'b self,
        bound: &'b TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> + 'b {
        self.whole_type_bound_targets()
            .map(move |ty| syn::parse_quote! { #ty: #bound })
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
