use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::WherePredicate;

use crate::context::internal::enums::{
    EnumIr, VariantIr, VariantShape, build_binding_ident, collect_recall_like_bounds_for_enum,
};
use crate::context::internal::shared::lifetime::is_phantom_data;
use crate::context::internal::shared::{CodegenEnv, CodegenItemIr, FieldIr, FieldStrategy};

#[must_use]
pub(crate) fn gen_enum_recallable_impl(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let enum_type = ir.enum_type();
    let where_clause = build_enum_recallable_where_clause(ir, env);
    let memento_type = ir.memento_type();
    let restore_helper = gen_enum_restore_helper(ir, env);

    quote! {
        #[automatically_derived]
        impl #impl_generics #recallable_trait
            for #enum_type
        #where_clause {
            type Memento = #memento_type;
        }

        #restore_helper
    }
}

fn build_enum_recallable_where_clause(ir: &EnumIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let extra_bounds = collect_enum_recallable_bounds(ir, env);
    ir.extend_where_clause(extra_bounds)
}

fn collect_enum_recallable_bounds(ir: &EnumIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds_for_enum(ir, env, &env.recallable_trait)
}

fn gen_enum_restore_helper(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    if !ir.supports_derived_recall() {
        return quote! {};
    }

    let impl_generics = ir.impl_generics();
    let enum_type = ir.enum_type();
    let recallable_trait = &env.recallable_trait;
    let memento_name = ir.memento_name();
    let where_clause = build_enum_recallable_where_clause(ir, env);
    let arms = ir.variants().map(|variant| {
        let variant_name = variant.name;
        let pattern = build_variant_memento_pattern(variant);
        let expr = build_variant_restore_expr(variant, ir.name());
        quote! { #memento_name::#variant_name #pattern => #expr }
    });
    let marker_arm = ir.synthetic_marker_type().map(|_| {
        quote! { #memento_name::__RecallableMarker(_) => unreachable!("marker variant is never constructed"), }
    });

    quote! {
        impl #impl_generics #enum_type #where_clause {
            #[inline]
            fn __recallable_restore_from_memento(
                memento: <#enum_type as #recallable_trait>::Memento,
            ) -> Self {
                match memento {
                    #(#arms,)*
                    #marker_arm
                }
            }
        }
    }
}

fn build_variant_memento_pattern(variant: &VariantIr<'_>) -> TokenStream2 {
    let mut bindings = variant.kept_bindings().peekable();
    if bindings.peek().is_none() {
        return quote! {};
    }

    match variant.shape {
        VariantShape::Named => quote! { { #(#bindings),* } },
        VariantShape::Unnamed => quote! { ( #(#bindings),* ) },
        VariantShape::Unit => quote! {},
    }
}

fn build_variant_restore_expr(variant: &VariantIr<'_>, enum_name: &syn::Ident) -> TokenStream2 {
    let variant_name = variant.name;

    match variant.shape {
        VariantShape::Named => {
            let inits = variant.indexed_fields().map(|(index, field)| {
                let member = &field.member;
                let value = build_variant_restore_value(field, index);
                quote! { #member: #value }
            });
            quote! { #enum_name::#variant_name { #(#inits),* } }
        }
        VariantShape::Unnamed => {
            let values = variant
                .indexed_fields()
                .map(|(index, field)| build_variant_restore_value(field, index));
            quote! { #enum_name::#variant_name(#(#values),*) }
        }
        VariantShape::Unit => quote! { #enum_name::#variant_name },
    }
}

fn build_variant_restore_value(field: &FieldIr<'_>, index: usize) -> TokenStream2 {
    match field.strategy {
        FieldStrategy::StoreAsSelf => {
            let binding = build_binding_ident(field, index);
            quote! { #binding }
        }
        FieldStrategy::Skip if is_phantom_data(field.ty) => quote! { ::core::marker::PhantomData },
        FieldStrategy::StoreAsMemento | FieldStrategy::Skip => {
            unreachable!("manual-only gating rejects non-phantom skipped and recallable fields")
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::gen_enum_recallable_impl;
    use crate::context::internal::enums::EnumIr;
    use crate::context::internal::shared::CodegenEnv;

    #[test]
    fn helper_name_and_manual_only_guidance_helper_name() {
        let input: syn::DeriveInput = parse_quote! {
            enum Example {
                Value(u32),
            }
        };
        let ir = EnumIr::analyze(&input).unwrap();
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
        };

        let tokens = gen_enum_recallable_impl(&ir, &env).to_string();

        assert!(tokens.contains("__recallable_restore_from_memento"));
        assert!(!tokens.contains("__recallable_rebuild_from_memento"));
    }
}
