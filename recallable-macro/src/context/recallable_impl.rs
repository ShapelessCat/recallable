use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::MacroContext;
use crate::context::{CodegenEnv, StructIr};

impl<'a> MacroContext<'a> {
    // ============================================================
    // impl<T, ...> Recallable for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_recallable_trait_impl(&self) -> TokenStream2 {
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let recallable_trait = &self.recallable_trait;
        let input_struct_name = self.struct_name;
        let extra_trait_bounds = self.build_trait_bounds(recallable_trait);
        let where_clause = self.extend_where_clause(&extra_trait_bounds);
        let memento_struct_type = &self.memento_struct_type;

        quote! {
            impl #impl_generics #recallable_trait
                for #input_struct_name #type_generics
            #where_clause {
                type Memento = #memento_struct_type;
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn gen_recallable_impl(ir: &StructIr, env: &CodegenEnv) -> TokenStream2 {
    let (impl_generics, type_generics, _) = ir.generics.split_for_impl();
    let recallable_trait = &env.recallable_trait;
    let struct_name = ir.name;
    let extra_bounds = ir.recallable_bounds(recallable_trait);
    let where_clause = ir.extend_where_clause(&extra_bounds);
    let memento_type = ir.memento_type();

    quote! {
        impl #impl_generics #recallable_trait
            for #struct_name #type_generics
        #where_clause {
            type Memento = #memento_type;
        }
    }
}
