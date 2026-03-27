use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use syn::WherePredicate;

use crate::context::{
    CodegenEnv, EnumIr, EnumRecallMode, FieldIr, FieldMember, ItemIr, StructIr, VariantIr,
    VariantShape, collect_recall_like_bounds, collect_recall_like_bounds_for_enum,
};

#[must_use]
pub(crate) fn gen_recallable_impl(ir: &ItemIr, env: &CodegenEnv) -> TokenStream2 {
    match ir {
        ItemIr::Struct(ir) => gen_struct_recallable_impl(ir, env),
        ItemIr::Enum(ir) => gen_enum_recallable_impl(ir, env),
    }
}

fn gen_struct_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let struct_type = ir.struct_type();
    let where_clause = build_recallable_where_clause(ir, env);
    let memento_type = ir.memento_type();

    quote! {
        impl #impl_generics #recallable_trait
            for #struct_type
        #where_clause {
            type Memento = #memento_type;
        }
    }
}

fn build_recallable_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<syn::WhereClause> {
    let extra_bounds = collect_recallable_bounds(ir, env);
    ir.extend_where_clause(extra_bounds)
}

fn collect_recallable_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds(ir, env, &env.recallable_trait)
}

fn gen_enum_recallable_impl(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let enum_type = ir.enum_type();
    let where_clause = build_enum_recallable_where_clause(ir, env);
    let memento_type = ir.memento_type();
    let rebuild_helper = gen_enum_rebuild_helper(ir, env);

    quote! {
        impl #impl_generics #recallable_trait
            for #enum_type
        #where_clause {
            type Memento = #memento_type;
        }

        #rebuild_helper
    }
}

fn build_enum_recallable_where_clause(
    ir: &EnumIr,
    env: &CodegenEnv,
) -> Option<syn::WhereClause> {
    let extra_bounds = collect_enum_recallable_bounds(ir, env);
    ir.extend_where_clause(extra_bounds)
}

fn collect_enum_recallable_bounds(ir: &EnumIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds_for_enum(ir, env, &env.recallable_trait)
}

fn gen_enum_rebuild_helper(ir: &EnumIr, env: &CodegenEnv) -> TokenStream2 {
    if !matches!(ir.recall_mode(), EnumRecallMode::AssignmentOnly) {
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
        let expr = build_variant_rebuild_expr(variant, ir.name());
        quote! { #memento_name::#variant_name #pattern => #expr }
    });
    let marker_arm = ir.synthetic_marker_type().map(|_| {
        quote! { #memento_name::__RecallableMarker(_) => unreachable!("marker variant is never constructed"), }
    });

    quote! {
        impl #impl_generics #enum_type #where_clause {
            #[inline]
            fn __recallable_rebuild_from_memento(
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
    match variant.shape {
        VariantShape::Named => {
            let bindings = variant
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| build_binding_ident(field, index).to_token_stream());
            quote! { { #(#bindings),* } }
        }
        VariantShape::Unnamed => {
            let bindings = variant
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| build_binding_ident(field, index).to_token_stream());
            quote! { ( #(#bindings),* ) }
        }
        VariantShape::Unit => quote! {},
    }
}

fn build_variant_rebuild_expr(variant: &VariantIr<'_>, enum_name: &syn::Ident) -> TokenStream2 {
    let variant_name = variant.name;

    match variant.shape {
        VariantShape::Named => {
            let inits = variant.fields.iter().enumerate().map(|(index, field)| {
                let member = &field.member;
                let binding = build_binding_ident(field, index);
                quote! { #member: #binding }
            });
            quote! { #enum_name::#variant_name { #(#inits),* } }
        }
        VariantShape::Unnamed => {
            let values = variant
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| build_binding_ident(field, index).to_token_stream());
            quote! { #enum_name::#variant_name(#(#values),*) }
        }
        VariantShape::Unit => quote! { #enum_name::#variant_name },
    }
}

fn build_binding_ident(field: &FieldIr<'_>, index: usize) -> syn::Ident {
    match &field.member {
        FieldMember::Named(name) => (*name).clone(),
        FieldMember::Unnamed(_) => format_ident!("__recallable_field_{index}"),
    }
}
