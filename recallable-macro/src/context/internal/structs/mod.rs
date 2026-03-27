mod bounds;
mod ir;

pub(crate) use bounds::{collect_recall_like_bounds, collect_shared_memento_bounds};
pub(crate) use ir::{StructIr, StructShape};
