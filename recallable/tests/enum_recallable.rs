use core::marker::PhantomData;
use std::any::TypeId;

use recallable::Recallable;

#[derive(Clone, Debug, PartialEq, recallable::Recallable)]
struct GenericInner<T> {
    value: T,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable)]
enum NestedEnum<T> {
    Idle,
    Ready {
        #[recallable]
        inner: GenericInner<T>,
    },
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable)]
enum SkippedEnum<'a> {
    Idle,
    Borrowed {
        #[recallable(skip)]
        name: &'a str,
        value: u8,
    },
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable)]
enum PhantomEnum<'a, T> {
    Idle,
    Value {
        marker: PhantomData<&'a T>,
        value: T,
    },
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable)]
enum BoundDependentEnum<T: From<U>, U> {
    Value {
        value: T,
        #[recallable(skip)]
        marker: PhantomData<U>,
    },
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable)]
enum SkippedGenericEnum<T, U> {
    Value(T),
    Marker(#[recallable(skip)] PhantomData<U>),
}

#[test]
fn test_enum_with_recallable_field_derives_recallable() {
    fn assert_recallable<T: Recallable>() {}
    assert_recallable::<NestedEnum<u32>>();
}

#[test]
fn test_enum_with_skipped_field_derives_recallable() {
    type Memento = <SkippedEnum<'static> as Recallable>::Memento;
    let _ = core::any::type_name::<Memento>();
}

#[test]
fn test_enum_with_phantom_lifetime_derives_recallable() {
    type Memento = <PhantomEnum<'static, u8> as Recallable>::Memento;
    let _ = core::any::type_name::<Memento>();
}

#[test]
fn test_enum_memento_retains_bound_dependencies() {
    type Left = <BoundDependentEnum<String, &'static str> as Recallable>::Memento;
    type Right = <BoundDependentEnum<String, String> as Recallable>::Memento;

    assert_ne!(TypeId::of::<Left>(), TypeId::of::<Right>());
}

#[test]
fn test_enum_memento_prunes_skipped_generic_params() {
    type Left = <SkippedGenericEnum<u8, u16> as Recallable>::Memento;
    type Right = <SkippedGenericEnum<u8, u32> as Recallable>::Memento;

    assert_eq!(TypeId::of::<Left>(), TypeId::of::<Right>());
}

#[cfg(feature = "impl_from")]
#[test]
fn test_enum_from_impl_is_generated_for_recallable_only_enums() {
    let _: fn(NestedEnum<u32>) -> <NestedEnum<u32> as Recallable>::Memento =
        ::core::convert::From::from;
}
