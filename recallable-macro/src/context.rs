//! # Macro Context
//!
//! [`MacroContext::new`] parses the derive input and normalizes it into a
//! [`MacroContext`] that drives code generation.
//!
//! The context records field actions, preserved generics, and crate paths so the
//! macro can emit the companion memento struct plus the `Recallable` and `Recall`
//! trait implementations.

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
    Attribute, Data, DataStruct, DeriveInput, Field, Fields, GenericParam, Generics, Ident, Index,
    Meta, PathArguments, Type,
};

pub const IS_SERDE_ENABLED: bool = cfg!(feature = "serde");

const RECALLABLE: &str = "recallable";

#[derive(Debug)]
enum TypeUsage {
    NotRecallable,
    Recallable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldBehavior {
    Keep,
    Recall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructShape {
    Named,
    Unnamed,
    Unit,
}

impl StructShape {
    fn from_fields(fields: &Fields) -> Self {
        match fields {
            Fields::Named(_) => Self::Named,
            Fields::Unnamed(_) => Self::Unnamed,
            Fields::Unit => Self::Unit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecallPath {
    /// The entire field type implements `Recallable`.
    WholeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldStrategy {
    /// Field excluded from memento entirely.
    Skip,
    /// Field copied as-is into memento (type unchanged).
    StoreAsSelf,
    /// Field stored as its memento type, recalled recursively.
    StoreAsMemento(RecallPath),
}

impl FieldStrategy {
    pub(crate) fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeParamRetention {
    /// Used only by skipped fields — pruned from memento generics.
    Dropped,
    /// Used by kept fields — present in memento, no `Recallable` bound.
    Retained,
    /// Used by recalled fields — present in memento, needs `Recallable` bound.
    RetainedAsRecallable,
}

#[derive(Debug)]
pub(crate) struct TypeParamPlan<'a> {
    pub(crate) ident: &'a Ident,
    pub(crate) retention: TypeParamRetention,
}

#[derive(Debug)]
pub(crate) struct CodegenEnv {
    /// Base crate path (e.g. `::recallable`).
    #[allow(dead_code)]
    pub(crate) crate_path: TokenStream2,
    /// Fully qualified path to the `Recallable` trait.
    pub(crate) recallable_trait: TokenStream2,
    /// Fully qualified path to the `Recall` trait.
    pub(crate) recall_trait: TokenStream2,
    /// Whether the `serde` feature is enabled on the macro crate.
    pub(crate) serde_enabled: bool,
    /// Whether the `impl_from` feature is enabled on the macro crate.
    pub(crate) impl_from_enabled: bool,
}

#[derive(Debug)]
pub(crate) struct FieldIr<'a> {
    #[allow(dead_code)]
    pub(crate) source_index: usize,
    pub(crate) memento_index: Option<usize>,
    pub(crate) member: FieldMember<'a>,
    pub(crate) ty: &'a Type,
    pub(crate) strategy: FieldStrategy,
}

impl CodegenEnv {
    pub(crate) fn resolve() -> Self {
        let crate_path = crate_path();
        Self {
            recallable_trait: quote! { #crate_path::Recallable },
            recall_trait: quote! { #crate_path::Recall },
            crate_path,
            serde_enabled: IS_SERDE_ENABLED,
            impl_from_enabled: cfg!(feature = "impl_from"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct StructIr<'a> {
    pub(crate) name: &'a Ident,
    pub(crate) generics: &'a Generics,
    pub(crate) shape: StructShape,
    pub(crate) fields: Vec<FieldIr<'a>>,
    pub(crate) memento_name: Ident,
    pub(crate) type_params: Vec<TypeParamPlan<'a>>,
}

impl<'a> StructIr<'a> {
    pub(crate) fn analyze(input: &'a DeriveInput) -> syn::Result<Self> {
        let fields = extract_struct_fields(input)?;
        let struct_lifetimes = collect_struct_lifetimes(&input.generics);
        validate_no_borrowed_fields(fields, &struct_lifetimes)?;

        let shape = StructShape::from_fields(fields);
        let memento_name = quote::format_ident!("{}Memento", input.ident);

        let (type_params, field_irs) =
            collect_field_irs(fields, &struct_lifetimes, &input.generics)?;

        Ok(Self {
            name: &input.ident,
            generics: &input.generics,
            shape,
            fields: field_irs,
            memento_name,
            type_params,
        })
    }

    pub(crate) fn memento_params(&self) -> impl Iterator<Item = &Ident> {
        self.type_params.iter().filter_map(|p| {
            (!matches!(p.retention, TypeParamRetention::Dropped)).then_some(p.ident)
        })
    }

    pub(crate) fn recallable_params(&self) -> impl Iterator<Item = &Ident> {
        self.type_params.iter().filter_map(|p| {
            matches!(p.retention, TypeParamRetention::RetainedAsRecallable).then_some(p.ident)
        })
    }

    pub(crate) fn memento_type(&self) -> TokenStream2 {
        let name = &self.memento_name;
        let params: Vec<_> = self.memento_params().collect();
        if params.is_empty() {
            quote! { #name }
        } else {
            quote! { #name<#(#params),*> }
        }
    }

    pub(crate) fn memento_fields(&self) -> impl Iterator<Item = &FieldIr<'a>> {
        self.fields.iter().filter(|f| !f.strategy.is_skip())
    }

    pub(crate) fn recallable_bounds(&self, bound: &TokenStream2) -> Vec<syn::WherePredicate> {
        self.recallable_params()
            .map(|ty| syn::parse_quote! { #ty: #bound })
            .collect()
    }

    pub(crate) fn extend_where_clause(
        &self,
        extra: &[syn::WherePredicate],
    ) -> Option<syn::WhereClause> {
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

#[derive(Debug, Clone)]
pub(crate) enum FieldMember<'a> {
    Named(&'a Ident),
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
    let mut saw_recallable_attr = false;
    let mut saw_skip = false;

    for attr in field.attrs.iter().filter(|attr| is_recallable_attr(attr)) {
        saw_recallable_attr = true;
        match &attr.meta {
            Meta::Path(_) => {}
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

    Ok((!saw_skip).then_some(if saw_recallable_attr {
        FieldBehavior::Recall
    } else {
        FieldBehavior::Keep
    }))
}

fn field_member(field: &Field, index: usize) -> FieldMember<'_> {
    if let Some(field_name) = field.ident.as_ref() {
        FieldMember::Named(field_name)
    } else {
        FieldMember::Unnamed(Index::from(index))
    }
}

fn extract_recallable_type_name(field_type: &Type) -> syn::Result<Option<&Ident>> {
    match field_type {
        Type::Path(tp) if tp.qself.is_none() => {
            let segments = &tp.path.segments;
            if segments.len() == 1 {
                let segment = &segments[0];
                if matches!(segment.arguments, PathArguments::None) {
                    // Single bare ident — a generic type parameter.
                    Ok(Some(&segment.ident))
                } else {
                    // e.g. `Option<T>` — unsupported.
                    Err(syn::Error::new_spanned(
                        field_type,
                        "Only a simple generic type is supported here",
                    ))
                }
            } else {
                // Multi-segment path like `mod::Type` — concrete type, no generic param.
                Ok(None)
            }
        }
        _ => Err(syn::Error::new_spanned(
            field_type,
            "Only a simple generic type is supported here",
        )),
    }
}

fn record_non_recallable_type_usage<'a>(
    field_type: &'a Type,
    preserved_types: &mut HashMap<&'a Ident, TypeUsage>,
) {
    for type_name in collect_used_simple_types(field_type) {
        // Only mark as `NotRecallable` if not already marked as `Recallable`.
        preserved_types
            .entry(type_name)
            .or_insert(TypeUsage::NotRecallable);
    }
}

fn collect_field_irs<'a>(
    fields: &'a Fields,
    struct_lifetimes: &HashSet<&'a Ident>,
    generics: &'a Generics,
) -> syn::Result<(Vec<TypeParamPlan<'a>>, Vec<FieldIr<'a>>)> {
    let mut preserved_types = HashMap::new();
    let mut field_irs = Vec::with_capacity(fields.len());
    let mut memento_counter: usize = 0;

    for (index, field) in fields.iter().enumerate() {
        if is_phantom_data(&field.ty) && field_uses_struct_lifetime(&field.ty, struct_lifetimes) {
            field_irs.push(FieldIr {
                source_index: index,
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
                    source_index: index,
                    memento_index: None,
                    member: field_member(field, index),
                    ty: &field.ty,
                    strategy: FieldStrategy::Skip,
                });
            }
            Some(FieldBehavior::Keep) => {
                record_non_recallable_type_usage(&field.ty, &mut preserved_types);
                field_irs.push(FieldIr {
                    source_index: index,
                    memento_index: Some(memento_counter),
                    member: field_member(field, index),
                    ty: &field.ty,
                    strategy: FieldStrategy::StoreAsSelf,
                });
                memento_counter += 1;
            }
            Some(FieldBehavior::Recall) => {
                if let Some(type_name) = extract_recallable_type_name(&field.ty)? {
                    preserved_types.insert(type_name, TypeUsage::Recallable);
                }
                field_irs.push(FieldIr {
                    source_index: index,
                    memento_index: Some(memento_counter),
                    member: field_member(field, index),
                    ty: &field.ty,
                    strategy: FieldStrategy::StoreAsMemento(RecallPath::WholeType),
                });
                memento_counter += 1;
            }
        }
    }

    let type_params = generics
        .type_params()
        .map(|param| {
            let retention = match preserved_types.get(&param.ident) {
                Some(TypeUsage::Recallable) => TypeParamRetention::RetainedAsRecallable,
                Some(TypeUsage::NotRecallable) => TypeParamRetention::Retained,
                None => TypeParamRetention::Dropped,
            };
            TypeParamPlan {
                ident: &param.ident,
                retention,
            }
        })
        .collect();

    Ok((type_params, field_irs))
}

pub fn has_recallable_skip_attr(field: &Field) -> bool {
    // Use determine_field_behavior for consistent validation.
    // In the attribute macro context, we intentionally ignore errors here
    // because the derive macros will report them with proper spans.
    matches!(determine_field_behavior(field), Ok(None))
}

struct SimpleTypeCollector<'a> {
    used_simple_types: Vec<&'a Ident>,
}

impl<'ast> Visit<'ast> for SimpleTypeCollector<'ast> {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.qself.is_none()
            && let Some(segment) = node.path.segments.first()
        {
            self.used_simple_types.push(&segment.ident);
        }
        syn::visit::visit_type_path(self, node);
    }
}

fn collect_used_simple_types(ty: &Type) -> Vec<&Ident> {
    let mut collector = SimpleTypeCollector {
        used_simple_types: Vec::new(),
    };
    collector.visit_type(ty);
    collector.used_simple_types
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
