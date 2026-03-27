//! # Struct IR and Code Generation
//!
//! Semantic analysis and support logic live in the nested `internal` module.
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

pub(super) use from_impl::gen_from_impl;
pub(super) use internal::{
    CodegenEnv, EnumIr, EnumRecallMode, FieldIr, FieldMember, FieldStrategy, ItemIr, StructIr,
    StructShape, VariantIr, VariantShape, collect_recall_like_bounds,
    collect_recall_like_bounds_for_enum, collect_shared_memento_bounds,
    collect_shared_memento_bounds_for_enum, crate_path, has_recallable_skip_attr,
    is_generic_type_param,
};
pub(super) use recall_impl::gen_recall_impl;
pub(super) use recallable_impl::gen_recallable_impl;

pub(super) const SERDE_ENABLED: bool = cfg!(feature = "serde");
pub(super) const IMPL_FROM_ENABLED: bool = cfg!(feature = "impl_from");

pub(crate) fn gen_memento_type(ir: &ItemIr, env: &CodegenEnv) -> proc_macro2::TokenStream {
    memento::gen_memento_type(ir, env)
}

#[cfg(test)]
mod tests {
    use quote::ToTokens;
    use syn::parse_quote;

    use super::{CodegenEnv, ItemIr, gen_memento_type};

    #[test]
    fn split_internal_reexports_cover_both_item_kinds() {
        use crate::context::internal::{enums::EnumIr, shared::CodegenEnv, structs::StructIr};
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
        assert_eq!(
            crate::context::memento::gen_memento_type(&crate::context::ItemIr::Struct(struct_ir), &env)
                .to_string()
                .contains("ExampleMemento"),
            true
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

        let ir = ItemIr::analyze(&input).unwrap();
        let env = CodegenEnv::resolve();
        let memento: syn::ItemStruct = syn::parse2(gen_memento_type(&ir, &env)).unwrap();

        assert_eq!(memento.ident.to_string(), "ExampleMemento");
        assert_eq!(memento.vis.to_token_stream().to_string(), "pub (crate)");
    }
}
