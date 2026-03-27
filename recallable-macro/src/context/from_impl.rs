mod enums;
mod structs;

use proc_macro2::TokenStream as TokenStream2;

use crate::context::internal::shared::{CodegenEnv, ItemIr};

#[must_use]
pub(crate) fn gen_from_impl(ir: &ItemIr, env: &CodegenEnv) -> TokenStream2 {
    match ir {
        ItemIr::Struct(ir) => structs::gen_struct_from_impl(ir, env),
        ItemIr::Enum(ir) => enums::gen_enum_from_impl(ir, env),
    }
}
