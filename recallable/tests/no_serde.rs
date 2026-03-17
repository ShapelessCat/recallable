use core::marker::PhantomData;

use recallable::recallable_model;

mod nested {
    #[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
    pub struct Inner {
        pub value: i32,
    }
}

#[derive(recallable::Recallable, recallable::Recall)]
struct Wrapper<T> {
    #[recallable]
    value: nested::Inner,
    #[recallable(skip)]
    marker: PhantomData<T>,
}

const fn plus_one(x: i32) -> i32 {
    x + 1
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct PlainInner {
    value: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct PlainOuter<T> {
    #[recallable]
    inner: T,
    version: u32,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct DeriveOnlyStruct {
    value: i32,
    #[recallable(skip)]
    sticky: u32,
}

#[recallable_model]
#[derive(Clone, Debug)]
struct AllSkipped {
    #[recallable(skip)]
    marker: fn(i32) -> i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct FieldWithNonRecallableAttrBeforeSkip {
    value: i32,
    #[allow(dead_code)]
    #[recallable(skip)]
    sticky: u32,
}

#[test]
fn test_recallable_model_and_derive_generate_recall_types_without_serde() {
    fn assert_recallable<T: recallable::Recallable + recallable::Recall>() {}

    assert_recallable::<PlainInner>();
    assert_recallable::<PlainOuter<PlainInner>>();
    assert_recallable::<DeriveOnlyStruct>();
    assert_recallable::<AllSkipped>();
}

#[test]
fn test_recall_methods_are_generated_without_serde() {
    let _: fn(
        &mut PlainOuter<PlainInner>,
        <PlainOuter<PlainInner> as recallable::Recallable>::Memento,
    ) = <PlainOuter<PlainInner> as recallable::Recall>::recall;

    let _: fn(&mut DeriveOnlyStruct, <DeriveOnlyStruct as recallable::Recallable>::Memento) =
        <DeriveOnlyStruct as recallable::Recall>::recall;

    let _: fn(&mut AllSkipped, <AllSkipped as recallable::Recallable>::Memento) =
        <AllSkipped as recallable::Recall>::recall;

    let outer_memento_name =
        std::any::type_name::<<PlainOuter<PlainInner> as recallable::Recallable>::Memento>();
    let derive_memento_name =
        std::any::type_name::<<DeriveOnlyStruct as recallable::Recallable>::Memento>();
    assert!(outer_memento_name.contains("PlainOuter"));
    assert!(derive_memento_name.contains("DeriveOnlyStruct"));

    let value = AllSkipped { marker: plus_one };
    assert_eq!((value.marker)(1), 2);
}

#[test]
fn test_memento_types_implement_debug() {
    fn assert_debug<T: core::fmt::Debug>() {}
    assert_debug::<<PlainInner as recallable::Recallable>::Memento>();
    assert_debug::<<PlainOuter<PlainInner> as recallable::Recallable>::Memento>();
    assert_debug::<<DeriveOnlyStruct as recallable::Recallable>::Memento>();
    assert_debug::<<AllSkipped as recallable::Recallable>::Memento>();
}

#[test]
fn test_memento_types_implement_clone_and_partial_eq() {
    fn assert_clone_eq<T: Clone + PartialEq>() {}
    assert_clone_eq::<<PlainInner as recallable::Recallable>::Memento>();
    assert_clone_eq::<<PlainOuter<PlainInner> as recallable::Recallable>::Memento>();
    assert_clone_eq::<<DeriveOnlyStruct as recallable::Recallable>::Memento>();
}

#[test]
fn test_recallable_skip_works_with_non_recallable_field_attribute() {
    let _: fn(
        &mut FieldWithNonRecallableAttrBeforeSkip,
        <FieldWithNonRecallableAttrBeforeSkip as recallable::Recallable>::Memento,
    ) = <FieldWithNonRecallableAttrBeforeSkip as recallable::Recall>::recall;
}

#[test]
fn test_recallable_field_with_path_type_and_skipped_generic_param() {
    // Regression: `nested::Inner` (multi-segment path) must not be confused with
    // the generic type parameter `T`, even when they share the same last-segment name.
    fn assert_recallable<T: recallable::Recallable + recallable::Recall>() {}
    assert_recallable::<Wrapper<u32>>();
    assert_recallable::<Wrapper<nested::Inner>>();
}
