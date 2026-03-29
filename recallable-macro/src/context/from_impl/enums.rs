use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::WherePredicate;

use crate::context::internal::enums::{
    EnumIr, VariantIr, VariantShape, build_binding_ident, collect_shared_memento_bounds_for_enum,
};
use crate::context::internal::shared::{CodegenEnv, CodegenItemIr, FieldIr, build_from_value_expr};

#[must_use]
pub(crate) fn gen_enum_from_impl(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let enum_type = ir.enum_type();
    let memento_type = ir.memento_type();
    let where_clause = build_enum_from_where_clause(ir, env);
    let from_method = build_enum_from_method(ir);

    quote! {
        #[automatically_derived]
        impl #impl_generics ::core::convert::From<#enum_type>
            for #memento_type
        #where_clause {
            #from_method
        }
    }
}

fn build_enum_from_method(ir: &EnumIr) -> TokenStream2 {
    let enum_type = ir.enum_type();
    let fn_body = build_enum_from_body(ir);

    quote! {
        #[inline]
        fn from(value: #enum_type) -> Self {
            #fn_body
        }
    }
}

fn build_enum_from_body(ir: &EnumIr) -> TokenStream2 {
    let enum_name = ir.name();
    let memento_name = ir.memento_name();
    let arms = ir.variants().map(|variant| {
        let variant_name = variant.name;
        let pattern = build_variant_source_pattern(variant);
        let expr = build_variant_from_expr(variant);
        quote! { #enum_name::#variant_name #pattern => #memento_name::#variant_name #expr }
    });

    quote! {
        match value {
            #(#arms),*
        }
    }
}

fn build_variant_source_pattern(variant: &VariantIr<'_>) -> TokenStream2 {
    match variant.shape {
        VariantShape::Named => {
            let patterns = variant.indexed_fields().map(|(index, field)| {
                if field.strategy.is_skip() {
                    let member = &field.member;
                    quote! { #member: _ }
                } else {
                    build_binding_ident(field, index).to_token_stream()
                }
            });
            quote! { { #(#patterns),* } }
        }
        VariantShape::Unnamed => {
            let patterns = variant
                .indexed_fields()
                .map(|(index, field)| build_binding_pattern(field, index));
            quote! { ( #(#patterns),* ) }
        }
        VariantShape::Unit => quote! {},
    }
}

fn build_binding_pattern(field: &FieldIr<'_>, index: usize) -> TokenStream2 {
    if field.strategy.is_skip() {
        quote! { _ }
    } else {
        build_binding_ident(field, index).to_token_stream()
    }
}

fn build_variant_from_expr(variant: &VariantIr<'_>) -> Option<TokenStream2> {
    let mut kept_fields = variant.kept_fields().peekable();
    let non_empty = kept_fields.peek().is_some();
    match variant.shape {
        VariantShape::Named if non_empty => {
            let inits = kept_fields.map(|(index, field)| {
                let member = &field.member;
                let binding = build_binding_ident(field, index);
                let value = build_from_binding_expr(field, &binding);
                quote! { #member: #value }
            });
            Some(quote! { { #(#inits),* } })
        }
        VariantShape::Unnamed if non_empty => {
            let values = kept_fields.map(|(index, field)| {
                let binding = build_binding_ident(field, index);
                build_from_binding_expr(field, &binding)
            });
            Some(quote! { ( #(#values),* ) })
        }
        _ => None,
    }
}

fn build_from_binding_expr(field: &FieldIr<'_>, binding: &syn::Ident) -> TokenStream2 {
    build_from_value_expr(quote! { #binding }, field.strategy)
}

fn build_enum_from_where_clause(ir: &EnumIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let recallable_trait = &env.recallable_trait;
    ir.extend_where_clause(
        ir.recallable_params()
            .flat_map(|ty| -> [WherePredicate; 2] {
                [
                    syn::parse_quote! { #ty: #recallable_trait },
                    syn::parse_quote! { #ty::Memento: ::core::convert::From<#ty> },
                ]
            })
            .chain(collect_shared_memento_bounds_for_enum(ir, env))
            .chain(ir.whole_type_from_bounds(recallable_trait)),
    )
}
