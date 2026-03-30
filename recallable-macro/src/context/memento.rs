mod enums;
mod structs;

use proc_macro2::TokenStream as TokenStream2;

use crate::context::internal::serde_attrs::types::SerdeItemAttrs;
use crate::context::internal::shared::{CodegenEnv, ItemIr};

#[must_use]
pub(crate) fn gen_memento_type(ir: &ItemIr, env: &CodegenEnv, serde_attrs: &SerdeItemAttrs) -> TokenStream2 {
    match (ir, serde_attrs) {
        (ItemIr::Struct(ir), SerdeItemAttrs::Struct(attrs)) => {
            structs::gen_memento_struct(ir, env, attrs)
        }
        (ItemIr::Enum(ir), SerdeItemAttrs::Enum(attrs)) => {
            enums::gen_memento_enum(ir, env, attrs)
        }
        _ => unreachable!("item kind and serde attrs kind must match"),
    }
}
