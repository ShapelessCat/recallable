use core::marker::PhantomData;

use recallable::{Recall, Recallable, recallable_model};

mod path_nested {
    #[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
    pub struct Leaf {
        pub value: i32,
    }
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct PathWrapper<Leaf> {
    #[recallable]
    leaf: path_nested::Leaf,
    #[recallable(skip)]
    marker: PhantomData<Leaf>,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct Inner {
    value: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct Outer<InnerType> {
    #[recallable]
    inner: InnerType,
    extra: u32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct TupleOuter<InnerType>(#[recallable] InnerType, u32);

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct UnitOuter;

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct SkipOuter {
    value: i32,
    #[recallable(skip)]
    untouched: u32,
}

#[test]
fn test_from_struct_to_memento() {
    let original = Outer {
        inner: Inner { value: 42 },
        extra: 7,
    };

    let memento: <Outer<Inner> as Recallable>::Memento = original.clone().into();
    let mut target = Outer {
        inner: Inner { value: 0 },
        extra: 0,
    };

    target.recall(memento);
    assert_eq!(target, original);
}

#[test]
fn test_from_tuple_struct_to_memento() {
    let original = TupleOuter(Inner { value: 42 }, 7);
    let memento: <TupleOuter<Inner> as Recallable>::Memento = original.clone().into();
    let mut target = TupleOuter(Inner { value: 0 }, 0);

    target.recall(memento);
    assert_eq!(target, original);
}

#[test]
fn test_from_unit_struct_to_memento() {
    let memento: <UnitOuter as Recallable>::Memento = UnitOuter.into();
    let mut target = UnitOuter;

    target.recall(memento);
    assert_eq!(target, UnitOuter);
}

#[test]
fn test_from_recall_respects_skipped_fields() {
    let original = SkipOuter {
        value: 10,
        untouched: 7,
    };
    let memento: <SkipOuter as Recallable>::Memento = original.into();
    let mut target = SkipOuter {
        value: 0,
        untouched: 99,
    };

    target.recall(memento);
    assert_eq!(target.value, 10);
    assert_eq!(target.untouched, 99);
}

#[test]
fn test_recallable_field_with_multi_segment_path_type() {
    // Regression: `path_nested::Leaf` (multi-segment path) must not be confused with
    // the generic type parameter `T` even when the last segment name matches.
    let original = PathWrapper {
        leaf: path_nested::Leaf { value: 42 },
        marker: PhantomData::<u32>,
    };
    let memento: <PathWrapper<u32> as Recallable>::Memento = original.clone().into();
    let mut target = PathWrapper {
        leaf: path_nested::Leaf { value: 0 },
        marker: PhantomData::<u32>,
    };
    target.recall(memento);
    assert_eq!(target, original);
}
