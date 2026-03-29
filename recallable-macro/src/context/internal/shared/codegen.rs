use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident, Type, WhereClause, WherePredicate};

use super::fields::{FieldIr, FieldMember, FieldStrategy};
use super::generics::{GenericParamPlan, is_generic_type_param, marker_component};

pub(crate) trait CodegenItemIr<'a> {
    type Fields<'b>: Iterator<Item = &'b FieldIr<'a>>
    where
        Self: 'b,
        'a: 'b;

    fn generics(&self) -> &'a Generics;
    fn memento_name(&self) -> &Ident;
    fn generic_type_param_idents(&self) -> &HashSet<&'a Ident>;
    fn generic_params(&self) -> &[GenericParamPlan<'a>];
    fn marker_param_indices(&self) -> &[usize];
    fn all_fields(&self) -> Self::Fields<'_>;

    #[must_use]
    fn memento_decl_generics(&self) -> Option<TokenStream2> {
        let mut params = self
            .generic_params()
            .iter()
            .filter(|plan| plan.is_retained())
            .map(GenericParamPlan::decl_param)
            .peekable();

        params.peek().is_some().then_some(quote! { <#(#params),*> })
    }

    #[must_use]
    fn memento_type(&self) -> TokenStream2 {
        let name = self.memento_name();
        let mut args = self
            .generic_params()
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

    #[must_use]
    fn synthetic_marker_type(&self) -> Option<TokenStream2> {
        let marker_param_indices = self.marker_param_indices();
        if marker_param_indices.is_empty() {
            return None;
        }

        let generic_params = self.generic_params();
        let components = marker_param_indices
            .iter()
            .map(|&index| marker_component(generic_params[index].param));

        Some(quote! {
            ::core::marker::PhantomData<(#(#components,)*)>
        })
    }

    fn synthetic_marker_helper_defs(&self) -> impl Iterator<Item = TokenStream2> + '_ {
        self.marker_param_indices()
            .iter()
            .filter_map(|&index| const_marker_helper_def(self.generic_params()[index].param))
    }

    fn recallable_params<'b>(&'b self) -> impl Iterator<Item = &'a Ident> + 'b
    where
        'a: 'b,
    {
        self.generic_params()
            .iter()
            .filter_map(GenericParamPlan::recallable_ident)
    }

    fn recallable_bounds<'b>(
        &'b self,
        bound: &'b TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> + 'b
    where
        'a: 'b,
    {
        self.recallable_params()
            .map(move |ty| syn::parse_quote! { #ty: #bound })
    }

    fn recallable_memento_bounds<'b>(
        &'b self,
        bound: &'b TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> + 'b
    where
        'a: 'b,
    {
        self.recallable_params()
            .map(move |ty| syn::parse_quote! { #ty::Memento: #bound })
    }

    fn whole_type_bound_targets<'b>(&'b self) -> impl Iterator<Item = &'a Type> + 'b
    where
        'a: 'b,
    {
        let generic_type_param_idents = self.generic_type_param_idents();
        let mut seen = HashSet::new();

        self.all_fields()
            .filter_map(move |field| match field.strategy {
                FieldStrategy::StoreAsMemento
                    if !is_generic_type_param(field.ty, generic_type_param_idents)
                        && seen.insert(field.ty) =>
                {
                    Some(field.ty)
                }
                _ => None,
            })
    }

    fn whole_type_bounds<'b>(
        &'b self,
        bound: &'b TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> + 'b
    where
        'a: 'b,
    {
        self.whole_type_bound_targets()
            .map(move |ty| syn::parse_quote! { #ty: #bound })
    }

    fn whole_type_memento_bounds<'b>(
        &'b self,
        recallable_trait: &'b TokenStream2,
        bound: &'b TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> + 'b
    where
        'a: 'b,
    {
        self.whole_type_bound_targets()
            .map(move |ty| syn::parse_quote! { <#ty as #recallable_trait>::Memento: #bound })
    }

    fn whole_type_from_bounds<'b>(
        &'b self,
        recallable_trait: &'b TokenStream2,
    ) -> impl Iterator<Item = WherePredicate> + 'b
    where
        'a: 'b,
    {
        self.whole_type_bound_targets().flat_map(move |ty| {
            [
                syn::parse_quote! { #ty: #recallable_trait },
                syn::parse_quote! { <#ty as #recallable_trait>::Memento: ::core::convert::From<#ty> },
            ]
        })
    }

    fn extend_where_clause(
        &self,
        extra: impl IntoIterator<Item = WherePredicate>,
    ) -> Option<WhereClause> {
        let mut where_clause = self.generics().where_clause.clone();
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

fn const_marker_helper_ident(ident: &Ident) -> Ident {
    format_ident!("__RecallableConstMarker_{ident}")
}

fn const_marker_helper_def(param: &GenericParam) -> Option<TokenStream2> {
    match param {
        GenericParam::Const(param) => {
            let helper_ident = const_marker_helper_ident(&param.ident);
            let ty = &param.ty;

            Some(quote! {
                #[doc(hidden)]
                struct #helper_ident<const VALUE: #ty>;
            })
        }
        _ => None,
    }
}

#[must_use]
pub(crate) fn build_memento_field_ty(
    field: &FieldIr<'_>,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    let ty = field.ty;
    match field.strategy {
        FieldStrategy::StoreAsMemento => {
            if is_generic_type_param(ty, generic_type_params) {
                quote! { #ty::Memento }
            } else {
                quote! { <#ty as #recallable_trait>::Memento }
            }
        }
        FieldStrategy::StoreAsSelf => quote! { #ty },
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    }
}

#[must_use]
pub(crate) fn build_memento_field_tokens(
    field: &FieldIr<'_>,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    let field_ty = build_memento_field_ty(field, recallable_trait, generic_type_params);
    match &field.member {
        FieldMember::Named(name) => quote! { #name: #field_ty },
        FieldMember::Unnamed(_) => quote! { #field_ty },
    }
}

#[must_use]
pub(crate) fn build_from_value_expr(expr: TokenStream2, strategy: FieldStrategy) -> TokenStream2 {
    match strategy {
        FieldStrategy::StoreAsSelf => expr,
        FieldStrategy::StoreAsMemento => quote! { ::core::convert::From::from(#expr) },
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    }
}
