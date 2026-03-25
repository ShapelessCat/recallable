//! # Struct IR and Code Generation
//!
//! [`StructIr::analyze`] parses a `DeriveInput` into a [`StructIr`] — the
//! semantic intermediate representation that drives all code generation.
//!
//! [`CodegenEnv`] captures environment configuration (crate paths, feature
//! flags) resolved once per macro invocation.
//!
//! Code generation is split into free functions in submodules:
//! - [`gen_memento_struct`] — companion memento struct definition
//! - [`gen_recallable_impl`] — `Recallable` trait implementation
//! - [`gen_recall_impl`] — `Recall` trait implementation
//! - [`gen_from_impl`] — `From<Struct>` for memento (behind `impl_from` feature)

mod from_impl;
mod memento_struct;
mod recall_impl;
mod recallable_impl;

pub(crate) use from_impl::gen_from_impl;
pub(crate) use memento_struct::gen_memento_struct;
pub(crate) use recall_impl::gen_recall_impl;
pub(crate) use recallable_impl::gen_recallable_impl;

use std::collections::{HashMap, HashSet};

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::visit::Visit;
use syn::{
    Attribute, Data, DataStruct, DeriveInput, Field, Fields, GenericParam, Generics, Ident,
    ImplGenerics, Index, Meta, PathArguments, Type, Visibility, WhereClause, WherePredicate,
};

pub const IS_SERDE_ENABLED: bool = cfg!(feature = "serde");

const RECALLABLE: &str = "recallable";

/// Field-level behavior inferred from `#[recallable]` attributes during analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldBehavior {
    /// Keep the field in the memento with its original type.
    Keep,
    /// Store the field as an inner memento and this field can recalled recursively.
    Recall,
}

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
    pub(crate) const fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }
}

/// Whether a generic parameter is kept on the generated memento type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenericParamRetention {
    /// Omit the parameter from the generated memento declaration.
    Dropped,
    /// Retain the parameter because the memento shape depends on it.
    Retained,
    /// Retain the type parameter and later add `Recallable`-related bounds for it.
    RetainedAsRecallable,
}

#[derive(Debug)]
pub(crate) struct GenericParamPlan<'a> {
    pub(crate) param: &'a GenericParam,
    pub(crate) retention: GenericParamRetention,
}

impl<'a> GenericParamPlan<'a> {
    fn is_retained(&self) -> bool {
        !matches!(self.retention, GenericParamRetention::Dropped)
    }

    fn decl_param(&self) -> GenericParam {
        self.param.clone()
    }

    fn type_arg(&self) -> TokenStream2 {
        match self.param {
            GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                quote! { #lifetime }
            }
            GenericParam::Type(param) => {
                let ident = &param.ident;
                quote! { #ident }
            }
            GenericParam::Const(param) => {
                let ident = &param.ident;
                quote! { #ident }
            }
        }
    }

    fn recallable_ident(&self) -> Option<&'a Ident> {
        match (self.param, self.retention) {
            (GenericParam::Type(param), GenericParamRetention::RetainedAsRecallable) => {
                Some(&param.ident)
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CodegenEnv {
    /// Fully qualified path to the `Recallable` trait.
    pub(crate) recallable_trait: TokenStream2,
    /// Fully qualified path to the `Recall` trait.
    pub(crate) recall_trait: TokenStream2,
    /// Whether the `serde` feature is enabled on the macro crate.
    pub(crate) serde_enabled: bool,
    /// Whether the `impl_from` feature is enabled on the macro crate.
    pub(crate) impl_from_enabled: bool,
}

impl CodegenEnv {
    pub(crate) fn resolve() -> Self {
        let crate_path = crate_path();
        Self {
            recallable_trait: quote! { #crate_path::Recallable },
            recall_trait: quote! { #crate_path::Recall },
            serde_enabled: IS_SERDE_ENABLED,
            impl_from_enabled: cfg!(feature = "impl_from"),
        }
    }

    pub(crate) fn memento_trait_bounds(&self) -> TokenStream2 {
        quote! {
            ::core::clone::Clone
                + ::core::fmt::Debug
                + ::core::cmp::PartialEq
        }
    }

    pub(crate) fn deserialize_owned_bound(&self) -> Option<TokenStream2> {
        self.serde_enabled
            .then_some(quote! { ::serde::de::DeserializeOwned })
    }
}

#[derive(Debug)]
pub(crate) struct FieldIr<'a> {
    pub(crate) memento_index: Option<usize>,
    pub(crate) member: FieldMember<'a>,
    pub(crate) ty: &'a Type,
    pub(crate) strategy: FieldStrategy,
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

    pub(crate) fn impl_generics(&self) -> ImplGenerics<'_> {
        let (impl_generics, _, _) = self.generics.split_for_impl();
        impl_generics
    }

    pub(crate) fn generic_type_param_idents(&self) -> &HashSet<&'a Ident> {
        &self.generic_type_param_idents
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
        self.fields.iter().filter(|f| !f.strategy.is_skip())
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

    pub(crate) fn whole_type_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.whole_type_bound_targets()
            .into_iter()
            .map(|ty| syn::parse_quote! { #ty: #bound })
            .collect()
    }

    pub(crate) fn whole_type_memento_bounds(
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

fn collect_shared_memento_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    let recallable_trait = &env.recallable_trait;
    let memento_trait_bounds = env.memento_trait_bounds();

    let mut bounds = ir.recallable_memento_bounds(&memento_trait_bounds);
    bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
    if let Some(deserialize_owned) = env.deserialize_owned_bound() {
        bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
    }

    bounds
}

fn collect_recall_like_bounds(
    ir: &StructIr,
    env: &CodegenEnv,
    direct_bound: &TokenStream2,
) -> Vec<WherePredicate> {
    let shared_memento_bounds = collect_shared_memento_bounds(ir, env);
    let shared_param_bound_count = ir.recallable_params().count();

    let mut bounds = ir.recallable_bounds(direct_bound);
    bounds.extend(
        shared_memento_bounds
            .iter()
            .take(shared_param_bound_count)
            .cloned(),
    );
    bounds.extend(ir.whole_type_bounds(direct_bound));
    bounds.extend(
        shared_memento_bounds
            .into_iter()
            .skip(shared_param_bound_count),
    );
    bounds
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

/// Returns the path used to reference the `recallable` crate in generated code.
///
/// Uses `proc-macro-crate` to resolve the actual dependency name from `Cargo.toml`,
/// which handles crate renames (e.g., `my_recallable = { package = "recallable", ... }`).
///
/// Even when the macro expands inside the `recallable` crate itself, prefer the
/// absolute `::recallable` path instead of `crate`. That keeps doctests working:
/// rustdoc compiles them as external crates, so `crate` would point at the
/// temporary doctest crate rather than the real `recallable` library.
#[inline]
pub(super) fn crate_path() -> TokenStream2 {
    match crate_name("recallable") {
        Ok(FoundCrate::Itself) => quote! { ::recallable },
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote! { ::#ident }
        }
        Err(_) => quote! { ::recallable },
    }
}

#[inline]
fn is_recallable_attr(attr: &Attribute) -> bool {
    attr.path().is_ident(RECALLABLE)
}

fn extract_struct_fields(input: &DeriveInput) -> syn::Result<&Fields> {
    if let Data::Struct(DataStruct { fields, .. }) = &input.data {
        Ok(fields)
    } else {
        Err(syn::Error::new_spanned(
            input,
            "This derive macro can only be applied to structs",
        ))
    }
}

fn validate_no_borrowed_fields(
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

#[derive(Debug, Default)]
struct GenericUsage {
    retained: HashSet<usize>,
    recallable_type_params: HashSet<usize>,
}

#[derive(Debug)]
struct GenericParamLookup<'a> {
    type_params: HashMap<&'a Ident, usize>,
    const_params: HashMap<&'a Ident, usize>,
    lifetime_params: HashMap<&'a Ident, usize>,
}

impl<'a> GenericParamLookup<'a> {
    fn new(generics: &'a Generics) -> Self {
        let mut type_params = HashMap::new();
        let mut const_params = HashMap::new();
        let mut lifetime_params = HashMap::new();

        for (index, param) in generics.params.iter().enumerate() {
            match param {
                GenericParam::Lifetime(param) => {
                    lifetime_params.insert(&param.lifetime.ident, index);
                }
                GenericParam::Type(param) => {
                    type_params.insert(&param.ident, index);
                }
                GenericParam::Const(param) => {
                    const_params.insert(&param.ident, index);
                }
            }
        }

        Self {
            type_params,
            const_params,
            lifetime_params,
        }
    }

    fn type_param_index(&self, ident: &Ident) -> Option<usize> {
        self.type_params.get(ident).copied()
    }

    fn const_param_index(&self, ident: &Ident) -> Option<usize> {
        self.const_params.get(ident).copied()
    }
}

// Analysis-only classification for recalled field types.

struct BareTypeParam(usize);

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

fn collect_field_irs<'a>(
    fields: &'a Fields,
    struct_lifetimes: &HashSet<&'a Ident>,
    generic_lookup: &GenericParamLookup<'a>,
) -> syn::Result<(GenericUsage, Vec<FieldIr<'a>>)> {
    let mut usage = GenericUsage::default();
    let mut field_irs = Vec::with_capacity(fields.len());
    let mut memento_counter: usize = 0;

    for (index, field) in fields.iter().enumerate() {
        if is_phantom_data(&field.ty) && field_uses_struct_lifetime(&field.ty, struct_lifetimes) {
            field_irs.push(FieldIr {
                memento_index: None,
                member: field_member(field, index),
                ty: &field.ty,
                strategy: FieldStrategy::Skip,
            });
            continue;
        }

        match determine_field_behavior(field)? {
            None => {
                field_irs.push(FieldIr {
                    memento_index: None,
                    member: field_member(field, index),
                    ty: &field.ty,
                    strategy: FieldStrategy::Skip,
                });
            }
            Some(FieldBehavior::Keep) => {
                usage.retained.extend(collect_generic_dependencies_in_type(
                    &field.ty,
                    generic_lookup,
                ));
                field_irs.push(FieldIr {
                    memento_index: Some(memento_counter),
                    member: field_member(field, index),
                    ty: &field.ty,
                    strategy: FieldStrategy::StoreAsSelf,
                });
                memento_counter += 1;
            }
            Some(FieldBehavior::Recall) => {
                usage.retained.extend(collect_generic_dependencies_in_type(
                    &field.ty,
                    generic_lookup,
                ));
                if let Some(BareTypeParam(index)) =
                    classify_recallable_field_type(&field.ty, generic_lookup)?
                {
                    usage.recallable_type_params.insert(index);
                }
                field_irs.push(FieldIr {
                    memento_index: Some(memento_counter),
                    member: field_member(field, index),
                    ty: &field.ty,
                    strategy: FieldStrategy::StoreAsMemento,
                });
                memento_counter += 1;
            }
        }
    }

    Ok((usage, field_irs))
}

fn collect_marker_param_indices(
    fields: &[FieldIr<'_>],
    generic_params: &[GenericParamPlan<'_>],
    generic_lookup: &GenericParamLookup<'_>,
) -> Vec<usize> {
    let mut referenced_by_fields = HashSet::new();
    for field in fields.iter().filter(|field| !field.strategy.is_skip()) {
        referenced_by_fields.extend(collect_generic_dependencies_in_type(
            field.ty,
            generic_lookup,
        ));
    }

    generic_params
        .iter()
        .enumerate()
        .filter_map(|(index, plan)| {
            (plan.is_retained() && !referenced_by_fields.contains(&index)).then_some(index)
        })
        .collect()
}

fn plan_memento_generics<'a>(
    generics: &'a Generics,
    mut usage: GenericUsage,
    generic_lookup: &GenericParamLookup<'a>,
) -> (Vec<GenericParamPlan<'a>>, Option<WhereClause>) {
    let param_dependencies: Vec<_> = generics
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let mut deps = collect_generic_dependencies_in_param(param, generic_lookup);
            deps.remove(&index);
            deps
        })
        .collect();

    let predicates_with_deps: Vec<_> = generics
        .where_clause
        .as_ref()
        .map(|where_clause| {
            where_clause
                .predicates
                .iter()
                .map(|predicate| {
                    (
                        predicate.clone(),
                        collect_generic_dependencies_in_where_predicate(predicate, generic_lookup),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let mut kept_predicates = vec![false; predicates_with_deps.len()];

    loop {
        let mut changed = false;

        let retained_now: Vec<_> = usage.retained.iter().copied().collect();
        for index in retained_now {
            for dependency in &param_dependencies[index] {
                changed |= usage.retained.insert(*dependency);
            }
        }

        for (idx, (_, dependencies)) in predicates_with_deps.iter().enumerate() {
            if dependencies.is_empty() {
                continue;
            }
            if dependencies
                .iter()
                .any(|dependency| usage.retained.contains(dependency))
            {
                if !kept_predicates[idx] {
                    kept_predicates[idx] = true;
                    changed = true;
                }
                for dependency in dependencies {
                    changed |= usage.retained.insert(*dependency);
                }
            }
        }

        if !changed {
            break;
        }
    }

    let generic_params = generics
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let retention = if usage.retained.contains(&index) {
                if matches!(param, GenericParam::Type(_))
                    && usage.recallable_type_params.contains(&index)
                {
                    GenericParamRetention::RetainedAsRecallable
                } else {
                    GenericParamRetention::Retained
                }
            } else {
                GenericParamRetention::Dropped
            };
            GenericParamPlan { param, retention }
        })
        .collect();

    let memento_where_clause = generics.where_clause.clone().and_then(|mut where_clause| {
        where_clause.predicates = where_clause
            .predicates
            .into_iter()
            .enumerate()
            .filter_map(|(idx, predicate)| kept_predicates[idx].then_some(predicate))
            .collect();

        (!where_clause.predicates.is_empty()).then_some(where_clause)
    });

    (generic_params, memento_where_clause)
}

pub fn has_recallable_skip_attr(field: &Field) -> bool {
    // Use determine_field_behavior for consistent validation.
    // In the attribute macro context, we intentionally ignore errors here
    // because the derive macros will report them with proper spans.
    matches!(determine_field_behavior(field), Ok(None))
}

struct GenericDependencyCollector<'a> {
    lookup: &'a GenericParamLookup<'a>,
    dependencies: HashSet<usize>,
    angle_arg_depth: usize,
}

impl<'a> GenericDependencyCollector<'a> {
    fn new(lookup: &'a GenericParamLookup<'a>) -> Self {
        Self {
            lookup,
            dependencies: HashSet::new(),
            angle_arg_depth: 0,
        }
    }
}

impl<'ast, 'a> Visit<'ast> for GenericDependencyCollector<'a> {
    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        if let Some(index) = self.lookup.lifetime_params.get(&lifetime.ident).copied() {
            self.dependencies.insert(index);
        }
        syn::visit::visit_lifetime(self, lifetime);
    }

    fn visit_angle_bracketed_generic_arguments(
        &mut self,
        node: &'ast syn::AngleBracketedGenericArguments,
    ) {
        self.angle_arg_depth += 1;
        syn::visit::visit_angle_bracketed_generic_arguments(self, node);
        self.angle_arg_depth -= 1;
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.qself.is_none()
            && let Some(first_segment) = node.path.segments.first()
        {
            if let Some(index) = self.lookup.type_param_index(&first_segment.ident) {
                self.dependencies.insert(index);
            } else if self.angle_arg_depth > 0
                && node.path.segments.len() == 1
                && matches!(first_segment.arguments, PathArguments::None)
                && let Some(index) = self.lookup.const_param_index(&first_segment.ident)
            {
                // `syn` represents identity const arguments like `N` as `Type`.
                self.dependencies.insert(index);
            }
        }

        syn::visit::visit_type_path(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none() && node.path.segments.len() == 1 {
            let ident = &node.path.segments[0].ident;
            if let Some(index) = self.lookup.const_param_index(ident) {
                self.dependencies.insert(index);
            }
        }

        syn::visit::visit_expr_path(self, node);
    }
}

fn collect_generic_dependencies_in_type(
    ty: &Type,
    generic_lookup: &GenericParamLookup<'_>,
) -> HashSet<usize> {
    let mut collector = GenericDependencyCollector::new(generic_lookup);
    collector.visit_type(ty);
    collector.dependencies
}

fn collect_generic_dependencies_in_param(
    param: &GenericParam,
    generic_lookup: &GenericParamLookup<'_>,
) -> HashSet<usize> {
    let mut collector = GenericDependencyCollector::new(generic_lookup);
    collector.visit_generic_param(param);
    collector.dependencies
}

fn collect_generic_dependencies_in_where_predicate(
    predicate: &WherePredicate,
    generic_lookup: &GenericParamLookup<'_>,
) -> HashSet<usize> {
    let mut collector = GenericDependencyCollector::new(generic_lookup);
    collector.visit_where_predicate(predicate);
    collector.dependencies
}

fn marker_component(param: &GenericParam) -> TokenStream2 {
    match param {
        GenericParam::Lifetime(param) => {
            let lifetime = &param.lifetime;
            quote! { fn() -> &#lifetime () }
        }
        GenericParam::Type(param) => {
            let ident = &param.ident;
            quote! { fn() -> #ident }
        }
        GenericParam::Const(param) => {
            let ident = &param.ident;
            quote! { [(); { let _ = #ident; 0usize }] }
        }
    }
}

pub(super) fn is_generic_type_param(ty: &Type, generic_type_params: &HashSet<&Ident>) -> bool {
    match ty {
        Type::Path(tp) if tp.qself.is_none() && tp.path.segments.len() == 1 => {
            let segment = &tp.path.segments[0];
            matches!(segment.arguments, PathArguments::None)
                && generic_type_params.contains(&segment.ident)
        }
        _ => false,
    }
}

fn collect_struct_lifetimes(generics: &Generics) -> HashSet<&Ident> {
    generics
        .params
        .iter()
        .filter_map(|p| match p {
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

/// Heuristically detects PhantomData-shaped field types during analysis.
///
/// This matches any path type whose final segment is `PhantomData`, so it accepts
/// `PhantomData`, `marker::PhantomData`, `core::marker::PhantomData`,
/// `::core::marker::PhantomData`, `std::marker::PhantomData`, and
/// `::std::marker::PhantomData`.
///
/// Because proc macros cannot resolve types, this also intentionally matches any
/// user-defined type whose final path segment is `PhantomData`.
fn is_phantom_data(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "PhantomData")
    } else {
        false
    }
}

fn field_uses_struct_lifetime(ty: &Type, struct_lifetimes: &HashSet<&Ident>) -> bool {
    let mut checker = LifetimeUsageChecker {
        struct_lifetimes,
        found: false,
    };
    checker.visit_type(ty);
    checker.found
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens, quote};
    use syn::parse_quote;

    use super::{
        CodegenEnv, StructIr, classify_recallable_field_type, collect_recall_like_bounds,
        collect_shared_memento_bounds, gen_memento_struct, is_phantom_data,
    };

    #[test]
    fn memento_generics_preserve_retained_bounds_defaults_and_where_clauses() {
        let input = parse_quote! {
            struct Example<T: Clone = i32, U, const N: usize = 4>
            where
                T: ::core::convert::From<U>,
                U: Copy,
                [u8; N]: Copy,
            {
                value: T,
                bytes: [u8; N],
                #[recallable(skip)]
                marker: ::core::marker::PhantomData<U>,
            }
        };

        let ir = StructIr::analyze(&input).unwrap();

        assert_eq!(
            ir.memento_decl_generics().to_string(),
            quote!(<T: Clone = i32, U, const N: usize = 4>).to_string()
        );
        assert_eq!(
            ir.memento_where_clause()
                .unwrap()
                .to_token_stream()
                .to_string(),
            quote!(
                where
                    T: ::core::convert::From<U>,
                    U: Copy,
                    [u8; N]: Copy
            )
            .to_string()
        );
        assert_eq!(
            ir.memento_type().to_string(),
            quote!(ExampleMemento<T, U, N>).to_string()
        );
    }

    #[test]
    fn memento_where_clause_filters_predicates_for_dropped_params() {
        let input = parse_quote! {
            struct Example<T, U>
            where
                T: Clone,
                U: Copy,
            {
                value: T,
                #[recallable(skip)]
                marker: ::core::marker::PhantomData<U>,
            }
        };

        let ir = StructIr::analyze(&input).unwrap();

        assert_eq!(
            ir.memento_decl_generics().to_string(),
            quote!(<T>).to_string()
        );
        assert_eq!(
            ir.memento_where_clause()
                .unwrap()
                .to_token_stream()
                .to_string(),
            quote!(where T: Clone).to_string()
        );
        assert_eq!(
            ir.memento_type().to_string(),
            quote!(ExampleMemento<T>).to_string()
        );
    }

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
        let lookup = super::GenericParamLookup::new(&input.generics);

        assert!(matches!(
            classify_recallable_field_type(&field.ty, &lookup),
            Ok(None)
        ));
    }

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
    fn shared_bound_helpers_preserve_predicate_order() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<T, U, V> {
                #[recallable]
                current: T,
                #[recallable]
                history: Option<U>,
                extra: V,
            }
        };
        let ir = StructIr::analyze(&input).unwrap();
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
            serde_enabled: true,
            impl_from_enabled: true,
        };

        let shared_bounds: Vec<_> = collect_shared_memento_bounds(&ir, &env)
            .into_iter()
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        assert_eq!(
            shared_bounds,
            vec![
                quote!(T::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::serde::de::DeserializeOwned)
                    .to_string(),
            ]
        );

        let recall_like_bounds: Vec<_> = collect_recall_like_bounds(&ir, &env, &env.recall_trait)
            .into_iter()
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        assert_eq!(
            recall_like_bounds,
            vec![
                quote!(T: ::recallable::Recall).to_string(),
                quote!(T::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(Option<U>: ::recallable::Recall).to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Option<U> as ::recallable::Recallable>::Memento: ::serde::de::DeserializeOwned)
                    .to_string(),
            ]
        );
    }

    #[test]
    fn whole_type_bound_helpers_deduplicate_repeated_types() {
        let input: syn::DeriveInput = parse_quote! {
            struct Example<T> {
                #[recallable]
                current: T,
                #[recallable]
                first: Wrapper<T>,
                #[recallable]
                second: Wrapper<T>,
            }
        };
        let ir = StructIr::analyze(&input).unwrap();
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
            serde_enabled: true,
            impl_from_enabled: true,
        };

        let whole_type_bounds: Vec<_> = ir
            .whole_type_bounds(&env.recallable_trait)
            .into_iter()
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        assert_eq!(
            whole_type_bounds,
            vec![quote!(Wrapper<T>: ::recallable::Recallable).to_string()]
        );

        let memento_trait_bounds = env.memento_trait_bounds();
        let whole_type_memento_bounds: Vec<_> = ir
            .whole_type_memento_bounds(&env.recallable_trait, &memento_trait_bounds)
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        assert_eq!(
            whole_type_memento_bounds,
            vec![
                quote!(<Wrapper<T> as ::recallable::Recallable>::Memento:
                    ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq)
                .to_string()
            ]
        );

        let whole_type_from_bounds: Vec<_> = ir
            .whole_type_from_bounds(&env.recallable_trait)
            .map(|predicate| predicate.to_token_stream().to_string())
            .collect();
        let wrapper_memento_from_bound: syn::WherePredicate = parse_quote! {
            <Wrapper<T> as ::recallable::Recallable>::Memento:
                ::core::convert::From<Wrapper<T>>
        };
        assert_eq!(
            whole_type_from_bounds,
            vec![
                quote!(Wrapper<T>: ::recallable::Recallable).to_string(),
                wrapper_memento_from_bound.to_token_stream().to_string(),
            ]
        );

        let recall_like_bounds: Vec<_> =
            collect_recall_like_bounds(&ir, &env, &env.recallable_trait)
                .into_iter()
                .map(|predicate| predicate.to_token_stream().to_string())
                .collect();
        assert_eq!(
            recall_like_bounds,
            vec![
                quote!(T: ::recallable::Recallable).to_string(),
                quote!(T::Memento: ::core::clone::Clone
                    + ::core::fmt::Debug
                    + ::core::cmp::PartialEq)
                .to_string(),
                quote!(Wrapper<T>: ::recallable::Recallable).to_string(),
                quote!(<Wrapper<T> as ::recallable::Recallable>::Memento:
                    ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq)
                .to_string(),
                quote!(<Wrapper<T> as ::recallable::Recallable>::Memento:
                    ::serde::de::DeserializeOwned)
                .to_string(),
            ]
        );
    }

    #[test]
    fn generated_memento_visibility_matches_companion_struct() {
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
            serde_enabled: true,
            impl_from_enabled: true,
        };

        let restricted_input: syn::DeriveInput = parse_quote! {
            pub(crate) struct Example {
                value: u32,
            }
        };
        let restricted_ir = StructIr::analyze(&restricted_input).unwrap();
        let restricted_memento: syn::ItemStruct =
            syn::parse2(gen_memento_struct(&restricted_ir, &env)).unwrap();
        assert_eq!(
            restricted_memento.vis.to_token_stream().to_string(),
            quote!(pub(crate)).to_string()
        );

        let private_input: syn::DeriveInput = parse_quote! {
            struct PrivateExample {
                value: u32,
            }
        };
        let private_ir = StructIr::analyze(&private_input).unwrap();
        let private_memento: syn::ItemStruct =
            syn::parse2(gen_memento_struct(&private_ir, &env)).unwrap();
        assert!(matches!(private_memento.vis, syn::Visibility::Inherited));
    }
}
