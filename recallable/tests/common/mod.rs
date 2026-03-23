use proptest::{array::uniform3, prelude::*};
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

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyInner {
    pub enabled: bool,
    pub lanes: [u8; 3],
}

pub type PropertyInnerMemento = <PropertyInner as Recallable>::Memento;

// Property tests use a small nested model with one skipped field so they can
// simultaneously exercise nested mementos, scalar fields, and skip semantics
// across both backends without requiring alloc-backed collections.
#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyOuter {
    pub level: i16,
    pub threshold: u32,
    #[recallable]
    pub nested: PropertyInner,
    #[recallable(skip)]
    pub skipped_marker: u8,
}

pub type PropertyOuterMemento = <PropertyOuter as Recallable>::Memento;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct InspectablePropertyOuterMemento {
    pub level: i16,
    pub threshold: u32,
    pub nested: PropertyInnerMemento,
}

// Generate broad but backend-friendly inputs so property tests cover more than
// a single happy-path example while still matching the project's feature set.
pub fn property_outer_strategy() -> impl Strategy<Value = PropertyOuter> {
    (
        any::<i16>(),
        any::<u32>(),
        any::<bool>(),
        uniform3(any::<u8>()),
        any::<u8>(),
    )
        .prop_map(
            |(level, threshold, enabled, lanes, skipped_marker)| PropertyOuter {
                level,
                threshold,
                nested: PropertyInner { enabled, lanes },
                skipped_marker,
            },
        )
}

// Build a known target state whose skipped field is caller-controlled. The
// property tests use this to prove recall updates persisted fields only.
pub fn property_seed(skipped_marker: u8) -> PropertyOuter {
    PropertyOuter {
        level: 0,
        threshold: 0,
        nested: PropertyInner {
            enabled: false,
            lanes: [0, 0, 0],
        },
        skipped_marker,
    }
}

// Concrete schema versions keep compatibility assertions explicit: added,
// removed, and renamed fields each represent a different kind of drift.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDriftV1 {
    pub id: u8,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDriftAddedFieldV2 {
    pub id: u8,
    pub revision: u8,
    pub active: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDriftRemovedFieldV2 {
    pub id: u8,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDriftRenamedFieldV2 {
    pub id: u8,
    pub is_active: bool,
}
