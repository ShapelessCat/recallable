//! Semantic analysis and shared helper backend for the `context` codegen facade.

mod bounds;
mod fields;
mod generics;
mod ir;
mod lifetime;
mod util;

pub(crate) use bounds::{
    MementoTraitSpec, collect_recall_like_bounds, collect_shared_memento_bounds,
};
pub(crate) use fields::has_recallable_skip_attr;
pub(crate) use generics::is_generic_type_param;
pub(crate) use ir::{CodegenEnv, FieldIr, FieldMember, FieldStrategy, StructIr, StructShape};
pub(crate) use util::crate_path;
