use recallable::{Recallable, recallable_model};
use serde::{Deserialize, Serialize};

pub const fn identity(x: &i32) -> i32 {
    *x
}

#[recallable_model]
#[derive(Clone, Default, Debug, PartialEq)]
pub struct FakeMeasurement<T, ClosureType> {
    pub v: T,
    #[recallable(skip)]
    pub how: ClosureType,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeasurementResult<T>(pub T);

#[recallable_model]
#[derive(Clone, Debug)]
pub struct ScopedMeasurement<ScopeType, MeasurementType, MeasurementOutput> {
    pub current_control_level: ScopeType,
    #[recallable]
    pub inner: MeasurementType,
    pub current_base: MeasurementResult<MeasurementOutput>,
}

pub type ScopedMeasurementMemento =
    <ScopedMeasurement<u32, FakeMeasurement<i32, fn(&i32) -> i32>, i32> as Recallable>::Memento;

#[recallable_model]
#[derive(Clone, Default, Debug)]
pub struct SimpleStruct {
    pub val: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupleStruct(pub i32, pub u32);

#[recallable_model]
#[derive(Clone, Debug)]
pub struct TupleStructWithSkippedMiddle<F>(pub i32, #[recallable(skip)] pub F, pub i64);

pub type TupleStructWithSkippedMiddleMemento =
    <TupleStructWithSkippedMiddle<fn(i32) -> i32> as Recallable>::Memento;

#[recallable_model]
#[derive(Clone, Debug)]
pub struct TupleStructWithWhereClause<T>(pub i32, pub T, pub i64)
where
    T: From<(u32, u32)>;

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitStruct;

#[recallable_model]
#[derive(Clone, Debug)]
pub struct SkipSerializingStruct {
    #[recallable(skip)]
    pub skipped: i32,
    pub value: i32,
}

#[derive(Clone, Debug, Serialize, recallable::Recallable, recallable::Recall)]
pub struct DeriveOnlySkipBehavior {
    #[recallable(skip)]
    pub hidden: i32,
    pub shown: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Counter {
    pub value: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixedGenericUsage<T, H> {
    pub history: H,
    #[recallable]
    pub current: T,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingWhereTrailing<T, U>
where
    U: Default,
{
    #[recallable]
    pub inner: T,
    pub marker: U,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingWhereNoTrailing<T>
where
    T: Clone,
{
    #[recallable]
    pub inner: T,
}
