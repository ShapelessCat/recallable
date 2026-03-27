//! Semantic analysis and shared helper backend for the `context` codegen facade.

mod bounds;
mod fields;
mod generics;
mod ir;
mod lifetime;
mod util;

pub(crate) use bounds::{
    collect_recall_like_bounds, collect_recall_like_bounds_for_enum,
    collect_shared_memento_bounds, collect_shared_memento_bounds_for_enum,
};
pub(crate) use fields::has_recallable_skip_attr;
pub(crate) use generics::is_generic_type_param;
pub(crate) use ir::{
    CodegenEnv, EnumIr, EnumRecallMode, FieldIr, FieldMember, FieldStrategy, ItemIr, StructIr,
    StructShape, VariantIr, VariantShape,
};
pub(crate) use util::crate_path;
