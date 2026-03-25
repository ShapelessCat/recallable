use proptest::{array::uniform3, prelude::*};
use recallable::{Recallable, recallable_model};
use serde::{Deserialize, Serialize};

pub(crate) const fn identity(x: &i32) -> i32 {
    *x
}

#[recallable_model]
#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct FakeMeasurement<T, ClosureType> {
    pub(crate) v: T,
    #[recallable(skip)]
    pub(crate) how: ClosureType,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct MeasurementResult<T>(pub(crate) T);

#[recallable_model]
#[derive(Clone, Debug)]
pub(crate) struct ScopedMeasurement<ScopeType, MeasurementType, MeasurementOutput> {
    pub(crate) current_control_level: ScopeType,
    #[recallable]
    pub(crate) inner: MeasurementType,
    pub(crate) current_base: MeasurementResult<MeasurementOutput>,
}

pub(crate) type ScopedMeasurementMemento =
    <ScopedMeasurement<u32, FakeMeasurement<i32, fn(&i32) -> i32>, i32> as Recallable>::Memento;

#[recallable_model]
#[derive(Clone, Default, Debug)]
pub(crate) struct SimpleStruct {
    pub(crate) val: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TupleStruct(pub(crate) i32, pub(crate) u32);

#[recallable_model]
#[derive(Clone, Debug)]
pub(crate) struct TupleStructWithSkippedMiddle<F>(
    pub(crate) i32,
    #[recallable(skip)] pub(crate) F,
    pub(crate) i64,
);

pub(crate) type TupleStructWithSkippedMiddleMemento =
    <TupleStructWithSkippedMiddle<fn(i32) -> i32> as Recallable>::Memento;

#[recallable_model]
#[derive(Clone, Debug)]
pub(crate) struct TupleStructWithWhereClause<T>(pub(crate) i32, pub(crate) T, pub(crate) i64)
where
    T: From<(u32, u32)>;

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnitStruct;

#[recallable_model]
#[derive(Clone, Debug)]
pub(crate) struct SkipSerializingStruct {
    #[recallable(skip)]
    pub(crate) skipped: i32,
    pub(crate) value: i32,
}

#[derive(Clone, Debug, Serialize, recallable::Recallable, recallable::Recall)]
pub(crate) struct DeriveOnlySkipBehavior {
    #[recallable(skip)]
    pub(crate) hidden: i32,
    pub(crate) shown: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct Counter {
    pub(crate) value: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MixedGenericUsage<T, H> {
    pub(crate) history: H,
    #[recallable]
    pub(crate) current: T,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExistingWhereTrailing<T, U>
where
    U: Default,
{
    #[recallable]
    pub(crate) inner: T,
    pub(crate) marker: U,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExistingWhereNoTrailing<T>
where
    T: Clone,
{
    #[recallable]
    pub(crate) inner: T,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PropertyInner {
    pub(crate) enabled: bool,
    pub(crate) lanes: [u8; 3],
}

pub(crate) type PropertyInnerMemento = <PropertyInner as Recallable>::Memento;

// Property tests use a small nested model with one skipped field so they can
// simultaneously exercise nested mementos, scalar fields, and skip semantics
// across both backends without requiring alloc-backed collections.
#[recallable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PropertyOuter {
    pub(crate) level: i16,
    pub(crate) threshold: u32,
    #[recallable]
    pub(crate) nested: PropertyInner,
    #[recallable(skip)]
    pub(crate) skipped_marker: u8,
}

pub(crate) type PropertyOuterMemento = <PropertyOuter as Recallable>::Memento;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub(crate) struct InspectablePropertyOuterMemento {
    pub(crate) level: i16,
    pub(crate) threshold: u32,
    pub(crate) nested: PropertyInnerMemento,
}

// Generate broad but backend-friendly inputs so property tests cover more than
// a single happy-path example while still matching the project's feature set.
pub(crate) fn property_outer_strategy() -> impl Strategy<Value = PropertyOuter> {
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
pub(crate) fn property_seed(skipped_marker: u8) -> PropertyOuter {
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
pub(crate) struct SchemaDriftV1 {
    pub(crate) id: u8,
    pub(crate) active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SchemaDriftAddedFieldV2 {
    pub(crate) id: u8,
    pub(crate) revision: u8,
    pub(crate) active: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SchemaDriftRemovedFieldV2 {
    pub(crate) id: u8,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SchemaDriftRenamedFieldV2 {
    pub(crate) id: u8,
    pub(crate) is_active: bool,
}
