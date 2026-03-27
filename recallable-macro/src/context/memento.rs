mod enums;
mod structs;

use proc_macro2::TokenStream as TokenStream2;

use crate::context::internal::shared::{CodegenEnv, ItemIr};

#[must_use]
pub(crate) fn gen_memento_type(ir: &ItemIr, env: &CodegenEnv) -> TokenStream2 {
    match ir {
        ItemIr::Struct(ir) => structs::gen_memento_struct(ir, env),
        ItemIr::Enum(ir) => enums::gen_memento_enum(ir, env),
    }
}
