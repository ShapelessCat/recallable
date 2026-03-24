use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::{CodegenEnv, StructIr};

pub(crate) fn gen_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let impl_generics = ir.impl_generics();
    let recallable_trait = &env.recallable_trait;
    let memento_trait_bounds = quote! {
        ::core::clone::Clone
            + ::core::fmt::Debug
            + ::core::cmp::PartialEq
    };
    let struct_type = ir.struct_type();
    let where_clause = {
        let mut extra_bounds = ir.recallable_bounds(recallable_trait);
        extra_bounds.extend(ir.recallable_memento_bounds(&memento_trait_bounds));
        extra_bounds.extend(ir.whole_type_bounds(recallable_trait));
        extra_bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &memento_trait_bounds));
        if env.serde_enabled {
            let deserialize_owned = quote! { ::serde::de::DeserializeOwned };
            extra_bounds.extend(ir.whole_type_memento_bounds(recallable_trait, &deserialize_owned));
        }
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
