//! # Derive Backend Facade
//!
//! Semantic analysis and item-kind-specific support logic live in the nested
//! `internal` module.
//!
//! Code generation remains split into free functions in submodules:
//! - `gen_memento_type` — companion memento struct or enum definition
//! - [`gen_recallable_impl`] — `Recallable` trait implementation
//! - [`gen_recall_impl`] — `Recall` trait implementation
//! - [`gen_from_impl`] — `From<Item>` for memento (behind `impl_from` feature)

mod from_impl;
mod internal;
mod memento;
mod recall_impl;
mod recallable_impl;

use syn::DeriveInput;

use self::internal::shared::ItemIr;

pub(super) use from_impl::gen_from_impl;
pub(super) use internal::serde_attrs::parse::parse_recallable_serde_attrs;
pub(super) use internal::shared::{CodegenEnv, crate_path, has_recallable_skip_attr};
pub(super) use recall_impl::gen_recall_impl;
pub(super) use recallable_impl::gen_recallable_impl;

pub(super) const SERDE_ENABLED: bool = cfg!(feature = "serde");
pub(super) const IMPL_FROM_ENABLED: bool = cfg!(feature = "impl_from");

pub(super) fn analyze_item(input: &DeriveInput) -> syn::Result<ItemIr<'_>> {
    ItemIr::analyze(input)
}

pub(super) fn analyze_recall_input(input: &DeriveInput) -> syn::Result<ItemIr<'_>> {
    let ir = analyze_item(input)?;

    if let ItemIr::Enum(enum_ir) = &ir {
        enum_ir.ensure_recall_derive_allowed()?;
    }

    Ok(ir)
}

pub(super) fn analyze_model_input(input: &DeriveInput) -> syn::Result<()> {
    let ir = analyze_item(input)?;

    if let ItemIr::Enum(enum_ir) = &ir {
        enum_ir.ensure_model_derive_allowed()?;
    }

    Ok(())
}

pub(crate) fn gen_memento_type(
    ir: &ItemIr,
    env: &CodegenEnv,
    serde_attrs: &internal::serde_attrs::types::SerdeItemAttrs,
) -> proc_macro2::TokenStream {
    memento::gen_memento_type(ir, env, serde_attrs)
}

pub(super) fn analyze_serde_attrs(
    input: &DeriveInput,
) -> syn::Result<internal::serde_attrs::types::SerdeItemAttrs> {
    use internal::serde_attrs::types::SerdeItemAttrs;
    match &input.data {
        syn::Data::Struct(data) => {
            let attrs = internal::serde_attrs::analyze_struct_serde_attrs(&data.fields)?;
            Ok(SerdeItemAttrs::Struct(attrs))
        }
        syn::Data::Enum(data) => {
            let attrs = internal::serde_attrs::analyze_enum_serde_attrs(data)?;
            Ok(SerdeItemAttrs::Enum(attrs))
        }
        _ => unreachable!("unions rejected earlier"),
    }
}

#[cfg(test)]
mod tests {
    use quote::ToTokens;
    use syn::parse_quote;

    use super::{CodegenEnv, analyze_item, analyze_recall_input, gen_memento_type};
    use crate::context::internal::serde_attrs::types::{
        SerdeFieldAttrs, SerdeItemAttrs, SerdeStructAttrs,
    };

    #[test]
    fn split_internal_reexports_cover_both_item_kinds() {
        use crate::context::internal::{
            enums::EnumIr,
            shared::{CodegenEnv, ItemIr},
            structs::StructIr,
        };
        use syn::parse_quote;

        let struct_input: syn::DeriveInput = parse_quote! {
            struct Example<T> {
                value: T,
            }
        };
        let enum_input: syn::DeriveInput = parse_quote! {
            enum Choice<T> {
                One(T),
            }
        };

        let struct_ir = StructIr::analyze(&struct_input).unwrap();
        let enum_ir = EnumIr::analyze(&enum_input).unwrap();
        let env = CodegenEnv::resolve();

        assert_eq!(struct_ir.memento_name().to_string(), "ExampleMemento");
        assert_eq!(enum_ir.memento_name().to_string(), "ChoiceMemento");
        let serde_attrs = SerdeItemAttrs::Struct(SerdeStructAttrs {
            fields: vec![SerdeFieldAttrs::default()],
        });
        assert!(
            crate::context::memento::gen_memento_type(
                &ItemIr::Struct(struct_ir),
                &env,
                &serde_attrs
            )
            .to_string()
            .contains("ExampleMemento")
        );
    }

    #[test]
    fn helper_name_and_manual_only_guidance_manual_only_error() {
        let input: syn::DeriveInput = parse_quote! {
            enum Example {
                Value(#[recallable] Inner),
            }
        };

        let error = analyze_recall_input(&input).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("derive `Recallable` and implement `Recall` or `TryRecall` manually")
        );
    }

    #[test]
    fn facade_reexports_support_analysis_and_codegen() {
        let input: syn::DeriveInput = parse_quote! {
            pub(crate) struct Example<T> {
                #[recallable]
                value: T,
            }
        };

        let ir = analyze_item(&input).unwrap();
        let env = CodegenEnv::resolve();
        let serde_attrs = SerdeItemAttrs::Struct(SerdeStructAttrs {
            fields: vec![SerdeFieldAttrs::default()],
        });
        let memento: syn::ItemStruct =
            syn::parse2(gen_memento_type(&ir, &env, &serde_attrs)).unwrap();

        assert_eq!(memento.ident.to_string(), "ExampleMemento");
        assert_eq!(memento.vis.to_token_stream().to_string(), "pub (crate)");
    }

    #[test]
    fn analyze_item_rejects_unions_at_outer_boundary() {
        let input: syn::DeriveInput = parse_quote! {
            union Example {
                value: u32,
            }
        };

        let error = analyze_item(&input).unwrap_err();

        assert_eq!(
            error.to_string(),
            "This derive macro can only be applied to structs or enums"
        );
    }
}
