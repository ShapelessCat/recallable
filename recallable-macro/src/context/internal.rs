//! Semantic analysis and shared helper backend for the `context` codegen facade.

pub(crate) mod enums;
pub(crate) mod shared;
pub(crate) mod structs;

pub(crate) use enums::{
    EnumIr, EnumRecallMode, VariantIr, VariantShape, collect_recall_like_bounds_for_enum,
    collect_shared_memento_bounds_for_enum,
};
pub(crate) use shared::{
    CodegenEnv, FieldIr, FieldMember, FieldStrategy, ItemIr, crate_path,
    has_recallable_skip_attr, is_generic_type_param,
};
pub(crate) use structs::{
    StructIr, StructShape, collect_recall_like_bounds, collect_shared_memento_bounds,
};
