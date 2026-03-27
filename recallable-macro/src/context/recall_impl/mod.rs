mod enums;
mod structs;

use proc_macro2::TokenStream as TokenStream2;

use crate::context::{CodegenEnv, ItemIr};

#[must_use]
pub(crate) fn gen_recall_impl(ir: &ItemIr, env: &CodegenEnv) -> TokenStream2 {
    match ir {
        ItemIr::Struct(ir) => structs::gen_struct_recall_impl(ir, env),
        ItemIr::Enum(ir) => enums::gen_enum_recall_impl(ir, env),
    }
}
