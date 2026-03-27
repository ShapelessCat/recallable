pub(crate) mod bounds;
pub(crate) mod env;
pub(crate) mod fields;
pub(crate) mod generics;
pub(crate) mod item;
pub(crate) mod lifetime;
pub(crate) mod util;

pub(crate) use bounds::MementoTraitSpec;
pub(crate) use env::CodegenEnv;
pub(crate) use fields::{FieldIr, FieldMember, FieldStrategy, has_recallable_skip_attr};
pub(crate) use generics::is_generic_type_param;
pub(crate) use item::ItemIr;
pub(crate) use util::crate_path;
