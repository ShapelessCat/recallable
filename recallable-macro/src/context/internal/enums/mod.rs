mod bounds;
mod ir;

pub(crate) use bounds::{
    collect_recall_like_bounds_for_enum, collect_shared_memento_bounds_for_enum,
};
pub(crate) use ir::{EnumIr, VariantIr, VariantShape};
