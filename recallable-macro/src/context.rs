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
mod utils;

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
pub(crate) enum FieldBehavior {
    Keep,
    Recall,
}

#[derive(Debug)]
pub(crate) struct MacroContext<'a> {
    /// The name of the struct on which the derive macro is applied.
    struct_name: &'a Ident,
    /// The generics definition of the target struct.
    generics: &'a Generics,
    /// The fields of the target struct.
    fields: &'a Fields,
    /// Mapping from preserved type to its usage flag.
    preserved_types: HashMap<&'a Ident, TypeUsage>,
    /// The list of actions to perform for each field when generating the `recall` method and the
    /// memento struct.
    ///
    /// This determines whether a field is copied directly (`Keep`) or recursively recalled
    /// (`Recall`).
    field_actions: Vec<FieldAction<'a>>,
    /// The internal generated companion memento struct type (e.g., `MyStructMemento<T, ...>`).
    memento_struct_type: TokenStream2,
    /// Fully qualified path to the `Recallable` trait.
    recallable_trait: TokenStream2,
    /// Fully qualified path to the `Recall` trait.
    recall_trait: TokenStream2,
}

impl<'a> MacroContext<'a> {
    pub(crate) fn new(input: &'a DeriveInput) -> syn::Result<Self> {
        let fields = Self::extract_struct_fields(input)?;
        Self::validate_no_borrowed_fields(input, fields)?;
        let (preserved_types, field_actions) = Self::collect_field_actions(fields)?;
        let memento_struct_type =
            Self::build_memento_struct_type(&input.ident, &input.generics, &preserved_types);
        let crate_path = crate_path();
        let recallable_trait = quote! { #crate_path :: Recallable };
        let recall_trait = quote! { #crate_path :: Recall };

        Ok(Self {
            struct_name: &input.ident,
            generics: &input.generics,
            fields,
            preserved_types,
            field_actions,
            memento_struct_type,
            recallable_trait,
            recall_trait,
        })
    }

    fn validate_no_borrowed_fields(input: &DeriveInput, fields: &Fields) -> syn::Result<()> {
        let struct_lifetimes: HashSet<&Ident> = input
            .generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Lifetime(lt) => Some(&lt.lifetime.ident),
                _ => None,
            })
            .collect();

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
            if field_uses_struct_lifetime(&field.ty, &struct_lifetimes) {
                let err = syn::Error::new_spanned(
                    &field.ty,
                    "Recall derives do not support borrowed fields",
                );
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

    fn extract_struct_fields(input: &'a DeriveInput) -> syn::Result<&'a Fields> {
        if let Data::Struct(DataStruct { fields, .. }) = &input.data {
            Ok(fields)
        } else {
            Err(syn::Error::new_spanned(
                input,
                "This derive macro can only be applied to structs",
            ))
        }
    }

    fn collect_field_actions(
        fields: &'a Fields,
    ) -> syn::Result<(HashMap<&'a Ident, TypeUsage>, Vec<FieldAction<'a>>)> {
        let mut preserved_types = HashMap::new();
        let mut field_actions = Vec::with_capacity(fields.len());

        for (index, field) in fields.iter().enumerate() {
            Self::collect_field_action(index, field, &mut preserved_types, &mut field_actions)?;
        }

        Ok((preserved_types, field_actions))
    }

    fn collect_field_action(
        index: usize,
        field: &'a Field,
        preserved_types: &mut HashMap<&'a Ident, TypeUsage>,
        field_actions: &mut Vec<FieldAction<'a>>,
    ) -> syn::Result<()> {
        if let Some(field_behavior) = Self::determine_field_behavior(field)? {
            let member = Self::field_member(field, index);
            let field_type = &field.ty;
            match field_behavior {
                FieldBehavior::Recall => {
                    if let Some(type_name) = Self::extract_recallable_type_name(field_type)? {
                        // `Recallable` usage overrides `NotRecallable` usage.
                        preserved_types.insert(type_name, TypeUsage::Recallable);
                    }
                    // None means a concrete multi-segment path (e.g. `mod::Type`);
                    // no generic param to track.
                }
                FieldBehavior::Keep => {
                    Self::record_non_recallable_type_usage(field_type, preserved_types);
                }
            }
            field_actions.push(FieldAction {
                member,
                ty: field_type,
                behavior: field_behavior,
            });
        }
        Ok(())
    }

    pub(crate) fn determine_field_behavior(field: &Field) -> syn::Result<Option<FieldBehavior>> {
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

    fn field_member(field: &'a Field, index: usize) -> FieldMember<'a> {
        if let Some(field_name) = field.ident.as_ref() {
            FieldMember::Named(field_name)
        } else {
            FieldMember::Unnamed(Index::from(index))
        }
    }

    fn extract_recallable_type_name(field_type: &'a Type) -> syn::Result<Option<&'a Ident>> {
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

    fn record_non_recallable_type_usage(
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

    fn build_memento_struct_type(
        struct_name: &Ident,
        generics: &Generics,
        preserved_types: &HashMap<&'a Ident, TypeUsage>,
    ) -> TokenStream2 {
        let memento_struct_name = quote::format_ident!("{}Memento", struct_name);
        let memento_generic_params = generics.type_params().filter_map(|param| {
            preserved_types
                .contains_key(&param.ident)
                .then_some(&param.ident)
        });
        quote! { #memento_struct_name <#(#memento_generic_params),*> }
    }
}

#[derive(Debug)]
enum FieldMember<'a> {
    Named(&'a Ident),
    Unnamed(Index),
}

impl<'a> FieldMember<'a> {
    fn recall_member(&self, recall_index: usize) -> TokenStream2 {
        match self {
            FieldMember::Named(name) => quote! { #name },
            FieldMember::Unnamed(_) => {
                let index = Index::from(recall_index);
                quote! { #index }
            }
        }
    }
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
struct FieldAction<'a> {
    member: FieldMember<'a>,
    ty: &'a Type,
    behavior: FieldBehavior,
}

impl<'a> FieldAction<'a> {
    fn build_field(
        &self,
        recallable_trait: &TokenStream2,
        generic_type_params: &HashSet<&Ident>,
    ) -> TokenStream2 {
        let member = &self.member;
        let ty = self.ty;
        let field_ty = if self.behavior == FieldBehavior::Recall {
            if is_generic_type_param(ty, generic_type_params) {
                // Generic type param (e.g. `T`): use `T::Memento` so that derive macros on the
                // memento struct generate correct bounds like `T: Clone` rather than the
                // unsatisfied `<T as Recallable>::Memento: Clone`.
                quote! { #ty::Memento }
            } else {
                // Concrete type (e.g. `mod::Type` or `String`): use fully-qualified syntax to
                // avoid E0223 "ambiguous associated type".
                quote! { <#ty as #recallable_trait>::Memento }
            }
        } else {
            quote! { #ty }
        };
        match member {
            FieldMember::Named(name) => quote! { #name : #field_ty },
            FieldMember::Unnamed(_) => quote! { #field_ty },
        }
    }

    fn build_update_statement(
        &self,
        recall_trait: &TokenStream2,
        recall_index: usize,
    ) -> TokenStream2 {
        let member = &self.member;
        let recall_member = member.recall_member(recall_index);
        match self.behavior {
            FieldBehavior::Keep => {
                quote! { self.#member = memento.#recall_member; }
            }
            FieldBehavior::Recall => {
                quote! { #recall_trait::recall(&mut self.#member, memento.#recall_member); }
            }
        }
    }

    fn build_initializer_expr(&self) -> TokenStream2 {
        let member = &self.member;
        match self.behavior {
            FieldBehavior::Keep => quote! { value.#member },
            FieldBehavior::Recall => quote! { ::core::convert::From::from(value.#member) },
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

pub fn has_recallable_skip_attr(field: &Field) -> bool {
    // Use determine_field_behavior for consistent validation.
    // In the attribute macro context, we intentionally ignore errors here
    // because the derive macros will report them with proper spans.
    matches!(MacroContext::determine_field_behavior(field), Ok(None))
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

fn is_generic_type_param(ty: &Type, generic_type_params: &HashSet<&Ident>) -> bool {
    match ty {
        Type::Path(tp) if tp.qself.is_none() && tp.path.segments.len() == 1 => {
            let segment = &tp.path.segments[0];
            matches!(segment.arguments, PathArguments::None)
                && generic_type_params.contains(&segment.ident)
        }
        _ => false,
    }
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
