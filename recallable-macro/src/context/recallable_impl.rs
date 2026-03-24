use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::{CodegenEnv, StructIr};

pub(crate) fn gen_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let struct_type = ir.struct_type();
    let where_clause = {
        let extra_bounds = ir.recallable_bounds(recallable_trait);
        ir.extend_where_clause(&extra_bounds)
    };
    let memento_type = ir.memento_type();

    quote! {
        impl #impl_generics #recallable_trait
            for #struct_type
        #where_clause {
            type Memento = #memento_type;
        }
    }
}
