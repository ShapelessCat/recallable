use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{Fields, Ident};

use crate::context::{
    CodegenEnv, FieldIr, FieldMember, FieldStrategy, RecallPath, StructIr, StructShape,
};
use crate::{IS_SERDE_ENABLED, context::MacroContext};

impl<'a> MacroContext<'a> {
    // =================================================================================================
    // #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq, ::serde::Deserialize)]
    // struct InputTypeMemento<T, ...> ...
    // =================================================================================================

    pub(crate) fn build_memento_struct(&self) -> TokenStream2 {
        let derives = if IS_SERDE_ENABLED {
            quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq, ::serde::Deserialize)] }
        } else {
            quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq)] }
        };

        let memento_struct_type = &self.memento_struct_type;
        let bounded_types = self.build_trait_bounds(&self.recallable_trait);
        let where_clause = if bounded_types.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounded_types),* }
        };
        let recallable_trait = &self.recallable_trait;
        let generic_type_params: HashSet<&Ident> =
            self.generics.type_params().map(|p| &p.ident).collect();
        let recall_fields = self
            .field_actions
            .iter()
            .map(|action| action.build_field(recallable_trait, &generic_type_params));
        let body = match &self.fields {
            Fields::Named(_) => quote! { #where_clause { #(#recall_fields),* } },
            Fields::Unnamed(_) => quote! { ( #(#recall_fields),* ) #where_clause; },
            Fields::Unit => quote! {;},
        };
        quote! {
            #derives
            pub struct #memento_struct_type #body
        }
    }
}

#[allow(dead_code)]
pub(crate) fn gen_memento_struct(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let derives = if env.serde_enabled {
        quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq, ::serde::Deserialize)] }
    } else {
        quote! { #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::cmp::PartialEq)] }
    };

    let memento_type = ir.memento_type();
    let recallable_trait = &env.recallable_trait;
    let bounded_types = ir.recallable_bounds(recallable_trait);
    let where_clause = if bounded_types.is_empty() {
        quote! {}
    } else {
        quote! { where #(#bounded_types),* }
    };

    let generic_type_params: HashSet<&Ident> =
        ir.generics.type_params().map(|p| &p.ident).collect();

    let fields: Vec<_> = ir
        .memento_fields()
        .map(|field| build_memento_field_ir(field, recallable_trait, &generic_type_params))
        .collect();

    let body = match ir.shape {
        StructShape::Named => quote! { #where_clause { #(#fields),* } },
        StructShape::Unnamed => quote! { ( #(#fields),* ) #where_clause; },
        StructShape::Unit => quote! { ; },
    };

    quote! {
        #derives
        pub struct #memento_type #body
    }
}

#[allow(dead_code)]
fn build_memento_field_ir(
    field: &FieldIr,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    let ty = field.ty;
    let field_ty = match &field.strategy {
        FieldStrategy::StoreAsMemento(RecallPath::WholeType) => {
            if is_generic_type_param_ir(ty, generic_type_params) {
                quote! { #ty::Memento }
            } else {
                quote! { <#ty as #recallable_trait>::Memento }
            }
        }
        FieldStrategy::StoreAsSelf => quote! { #ty },
        FieldStrategy::Skip => unreachable!("memento_fields() filters skipped fields"),
    };
    match &field.member {
        FieldMember::Named(name) => quote! { #name: #field_ty },
        FieldMember::Unnamed(_) => quote! { #field_ty },
    }
}

#[allow(dead_code)]
fn is_generic_type_param_ir(ty: &syn::Type, generic_type_params: &HashSet<&Ident>) -> bool {
    match ty {
        syn::Type::Path(tp) if tp.qself.is_none() && tp.path.segments.len() == 1 => {
            let segment = &tp.path.segments[0];
            matches!(segment.arguments, syn::PathArguments::None)
                && generic_type_params.contains(&segment.ident)
        }
        _ => false,
    }
}
