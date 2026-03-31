use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{Ident, WhereClause, WherePredicate};

use crate::context::SERDE_ENABLED;
use crate::context::internal::serde_attrs::types::SerdeStructAttrs;
use crate::context::internal::shared::{
    CodegenEnv, CodegenItemIr, FieldIr, build_memento_field_tokens,
};
use crate::context::internal::structs::{StructIr, StructShape, collect_recall_like_bounds};

#[must_use]
pub(crate) fn gen_memento_struct(
    ir: &StructIr,
    env: &CodegenEnv,
    serde_attrs: &SerdeStructAttrs,
) -> TokenStream2 {
    let derives = ir.memento_trait_spec().derive_attr();
    let marker_helpers = ir.synthetic_marker_helper_defs();
    let visibility = ir.visibility();
    let memento_name = ir.memento_name();
    let memento_generics = ir.memento_decl_generics();
    let body = build_memento_body(ir, env, serde_attrs);

    quote! {
        #(#marker_helpers)*

        #[automatically_derived]
        #[allow(dead_code)]
        #derives
        #visibility struct #memento_name #memento_generics #body
    }
}

fn build_memento_body(
    ir: &StructIr,
    env: &CodegenEnv,
    serde_attrs: &SerdeStructAttrs,
) -> TokenStream2 {
    let shape = ir.generated_memento_shape();
    let where_clause = build_memento_where_clause(ir, env);
    let fields = memento_fields_with_marker(ir, env, shape, serde_attrs);

    match shape {
        StructShape::Named => quote! { #where_clause { #(#fields),* } },
        StructShape::Unnamed => quote! { ( #(#fields),* ) #where_clause; },
        StructShape::Unit => quote! { #where_clause; },
    }
}

fn build_memento_where_clause(ir: &StructIr, env: &CodegenEnv) -> Option<WhereClause> {
    let mut where_clause = ir
        .memento_where_clause()
        .cloned()
        .unwrap_or(syn::parse_quote! { where });
    let bounded_types = collect_memento_bounds(ir, env);
    where_clause.predicates.extend(bounded_types);

    if where_clause.predicates.is_empty() {
        None
    } else {
        Some(where_clause)
    }
}

fn collect_memento_bounds(ir: &StructIr, env: &CodegenEnv) -> Vec<WherePredicate> {
    collect_recall_like_bounds(ir, env, &env.recallable_trait)
}

fn memento_fields_with_marker<'ir, 'input>(
    ir: &'ir StructIr<'input>,
    env: &'ir CodegenEnv,
    shape: StructShape,
    serde_attrs: &'ir SerdeStructAttrs,
) -> impl Iterator<Item = TokenStream2> + 'ir {
    let recallable_trait = &env.recallable_trait;

    ir.memento_fields()
        .map(|field| {
            let serde_tokens = field
                .memento_index
                .map(|idx| serde_attrs.fields[idx].to_memento_tokens())
                .unwrap_or_default();
            let field_tokens =
                build_memento_field(field, recallable_trait, ir.generic_type_param_idents());
            quote! { #serde_tokens #field_tokens }
        })
        .chain(
            ir.synthetic_marker_type()
                .into_iter()
                .map(move |marker_ty| build_marker_field(&marker_ty, shape)),
        )
}

fn build_marker_field(marker_ty: &TokenStream2, shape: StructShape) -> TokenStream2 {
    let serde_attr = SERDE_ENABLED.then_some(quote! { #[serde(skip, default)] });

    match shape {
        StructShape::Named => quote! {
            #serde_attr
            _recallable_marker: #marker_ty
        },
        StructShape::Unnamed => quote! {
            #serde_attr
            #marker_ty
        },
        StructShape::Unit => unreachable!("unit mementos with synthetic markers become named"),
    }
}

fn build_memento_field(
    field: &FieldIr,
    recallable_trait: &TokenStream2,
    generic_type_params: &HashSet<&Ident>,
) -> TokenStream2 {
    build_memento_field_tokens(field, recallable_trait, generic_type_params)
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens, quote};
    use syn::parse_quote;

    use super::{CodegenEnv, StructIr, gen_memento_struct};
    use crate::context::internal::serde_attrs::types::{SerdeFieldAttrs, SerdeStructAttrs};

    #[test]
    fn generated_memento_visibility_matches_companion_struct() {
        let env = CodegenEnv {
            recallable_trait: quote!(::recallable::Recallable),
            recall_trait: quote!(::recallable::Recall),
        };

        let restricted_input: syn::DeriveInput = parse_quote! {
            pub(crate) struct Example {
                value: u32,
            }
        };
        let restricted_ir = StructIr::analyze(&restricted_input).unwrap();
        let serde = SerdeStructAttrs {
            fields: vec![SerdeFieldAttrs::default()],
        };
        let restricted_memento: syn::ItemStruct =
            syn::parse2(gen_memento_struct(&restricted_ir, &env, &serde)).unwrap();
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
        let serde = SerdeStructAttrs {
            fields: vec![SerdeFieldAttrs::default()],
        };
        let private_memento: syn::ItemStruct =
            syn::parse2(gen_memento_struct(&private_ir, &env, &serde)).unwrap();
        assert!(matches!(private_memento.vis, syn::Visibility::Inherited));
    }
}
