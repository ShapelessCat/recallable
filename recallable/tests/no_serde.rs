use core::marker::PhantomData;
use std::any::TypeId;

use recallable::recallable_model;
use serde::Deserialize;

mod nested {
    #[derive(
        Clone, Debug, PartialEq, serde::Deserialize, recallable::Recallable, recallable::Recall,
    )]
    pub(super) struct Inner {
        value: i32,
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

trait InlineState {
    type State: Clone + core::fmt::Debug + PartialEq + for<'de> Deserialize<'de>;
}

struct InlineStateProvider;

impl InlineState for InlineStateProvider {
    type State = i32;
}

#[derive(recallable::Recallable, recallable::Recall)]
struct InlineBoundOuter<T: InlineState> {
    value: <T as InlineState>::State,
}

#[derive(recallable::Recallable, recallable::Recall)]
struct FilteredWhereOuter<T, U>
where
    T: Clone,
    U: Copy,
{
    value: T,
    #[recallable(skip)]
    marker: PhantomData<U>,
}

#[derive(recallable::Recallable, recallable::Recall)]
struct DependentBoundOuter<T: From<U>, U> {
    value: T,
    #[recallable(skip)]
    marker: PhantomData<U>,
}

#[derive(recallable::Recallable, recallable::Recall)]
struct LifetimeBoundOuter<'a, T>
where
    T: Clone + core::fmt::Debug + PartialEq + From<&'a str>,
{
    value: T,
    #[recallable(skip)]
    marker: PhantomData<&'a str>,
}

#[derive(recallable::Recallable, recallable::Recall)]
struct ConstBoundOuter<T, const N: usize>
where
    T: Clone + core::fmt::Debug + PartialEq + From<ConstTag<N>>,
{
    value: T,
    #[recallable(skip)]
    marker: PhantomData<ConstTag<N>>,
}

#[derive(recallable::Recallable, recallable::Recall)]
struct ConcretePredicateOuter<T>
where
    T: Clone + core::fmt::Debug + PartialEq,
    [u8; 16]: Copy,
{
    value: T,
}

#[derive(Clone, Debug, PartialEq, Deserialize, recallable::Recallable, recallable::Recall)]
struct GenericInner<T> {
    value: T,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct GenericPathOuter {
    #[recallable]
    inner: GenericInner<u32>,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct OptionState<T>(Option<T>);

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct OptionPathOuter {
    #[recallable]
    inner: OptionState<u32>,
}

trait HasState {
    type State;
}

struct AssocProvider;

impl HasState for AssocProvider {
    type State = GenericInner<u32>;
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct AssocPathOuter {
    #[recallable]
    inner: <AssocProvider as HasState>::State,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct ConstTag<const N: usize> {
    value: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct GenericConstValue;

impl<const N: usize> From<ConstTag<N>> for GenericConstValue {
    fn from(_: ConstTag<N>) -> Self {
        Self
    }
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct ConstBuffer<const N: usize> {
    tag: ConstTag<N>,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct GenericPair<T, const N: usize> {
    value: T,
    tag: ConstTag<N>,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct ConstOuter<const N: usize> {
    #[recallable]
    inner: ConstBuffer<N>,
}

mod fixed_lengths {
    pub(super) const WIDTH: usize = 8;
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct FixedWidthBuffer {
    bytes: [u8; fixed_lengths::WIDTH],
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct MixedConstOuter<T, const N: usize> {
    #[recallable]
    inner: GenericPair<T, N>,
}

#[derive(recallable::Recallable, recallable::Recall)]
struct SkippedConstOuter<const N: usize> {
    value: u8,
    #[recallable(skip)]
    _marker: ConstTag<N>,
}

#[cfg(not(feature = "serde"))]
#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct ArrayBackedBuffer<const N: usize> {
    bytes: [u8; N],
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

#[test]
fn test_inline_generic_bounds_are_retained_on_mementos() {
    let _: fn(
        &mut InlineBoundOuter<InlineStateProvider>,
        <InlineBoundOuter<InlineStateProvider> as recallable::Recallable>::Memento,
    ) = <InlineBoundOuter<InlineStateProvider> as recallable::Recall>::recall;
}

#[test]
fn test_filtered_where_clause_drops_predicates_for_pruned_params() {
    type Left = <FilteredWhereOuter<u8, u16> as recallable::Recallable>::Memento;
    type Right = <FilteredWhereOuter<u8, u32> as recallable::Recallable>::Memento;

    assert_eq!(TypeId::of::<Left>(), TypeId::of::<Right>());
}

#[test]
fn test_bound_dependencies_keep_other_generic_params_retained() {
    type Left = <DependentBoundOuter<String, &'static str> as recallable::Recallable>::Memento;
    type Right = <DependentBoundOuter<String, String> as recallable::Recallable>::Memento;

    assert_ne!(TypeId::of::<Left>(), TypeId::of::<Right>());
}

#[test]
fn test_bound_dependencies_keep_lifetimes_on_memento_types() {
    // Regression: the retained where-clause lifetime `'a` must stay on the generated
    // memento type, and the skipped field means the synthetic marker must mention it
    // using the lifetime branch of `marker_component`.
    let _: fn(
        &mut LifetimeBoundOuter<'static, String>,
        <LifetimeBoundOuter<'static, String> as recallable::Recallable>::Memento,
    ) = <LifetimeBoundOuter<'static, String> as recallable::Recall>::recall;
}

#[test]
fn test_bound_dependencies_keep_const_params_on_memento_types() {
    // Regression: the retained const param `N` only survives via the skipped field,
    // so the synthetic marker must mention it using the const branch of
    // `marker_component`.
    let _: fn(
        &mut ConstBoundOuter<GenericConstValue, 2>,
        <ConstBoundOuter<GenericConstValue, 2> as recallable::Recallable>::Memento,
    ) = <ConstBoundOuter<GenericConstValue, 2> as recallable::Recall>::recall;

    type Left = <ConstBoundOuter<GenericConstValue, 1> as recallable::Recallable>::Memento;
    type Right = <ConstBoundOuter<GenericConstValue, 2> as recallable::Recallable>::Memento;

    assert_ne!(TypeId::of::<Left>(), TypeId::of::<Right>());
}

#[test]
fn test_concrete_only_where_predicates_are_accepted() {
    let _: fn(
        &mut ConcretePredicateOuter<String>,
        <ConcretePredicateOuter<String> as recallable::Recallable>::Memento,
    ) = <ConcretePredicateOuter<String> as recallable::Recall>::recall;
}

#[test]
fn test_recallable_field_accepts_generic_path_type() {
    let _: fn(&mut GenericPathOuter, <GenericPathOuter as recallable::Recallable>::Memento) =
        <GenericPathOuter as recallable::Recall>::recall;
}

#[test]
fn test_recallable_field_accepts_option_type() {
    let _: fn(&mut OptionPathOuter, <OptionPathOuter as recallable::Recallable>::Memento) =
        <OptionPathOuter as recallable::Recall>::recall;
}

#[test]
fn test_recallable_field_accepts_associated_type_paths() {
    let _: fn(&mut AssocPathOuter, <AssocPathOuter as recallable::Recallable>::Memento) =
        <AssocPathOuter as recallable::Recall>::recall;
}

#[test]
fn test_const_generic_struct_recall_works() {
    let _: fn(&mut ConstBuffer<3>, <ConstBuffer<3> as recallable::Recallable>::Memento) =
        <ConstBuffer<3> as recallable::Recall>::recall;
}

#[cfg(not(feature = "serde"))]
#[test]
fn test_const_generic_array_field_is_supported_without_serde() {
    let _: fn(
        &mut ArrayBackedBuffer<3>,
        <ArrayBackedBuffer<3> as recallable::Recallable>::Memento,
    ) = <ArrayBackedBuffer<3> as recallable::Recall>::recall;
}

#[test]
fn test_const_generic_recallable_field_is_supported() {
    let _: fn(&mut ConstOuter<2>, <ConstOuter<2> as recallable::Recallable>::Memento) =
        <ConstOuter<2> as recallable::Recall>::recall;
}

#[test]
fn test_const_generic_recallable_field_with_mixed_type_and_const_args() {
    let _: fn(
        &mut MixedConstOuter<u32, 2>,
        <MixedConstOuter<u32, 2> as recallable::Recallable>::Memento,
    ) = <MixedConstOuter<u32, 2> as recallable::Recall>::recall;
}

#[test]
fn test_multi_segment_const_paths_are_supported() {
    let _: fn(&mut FixedWidthBuffer, <FixedWidthBuffer as recallable::Recallable>::Memento) =
        <FixedWidthBuffer as recallable::Recall>::recall;
}

#[test]
fn test_skipped_only_const_params_are_pruned_from_memento_type() {
    type Left = <SkippedConstOuter<1> as recallable::Recallable>::Memento;
    type Right = <SkippedConstOuter<8> as recallable::Recallable>::Memento;

    assert_eq!(TypeId::of::<Left>(), TypeId::of::<Right>());
}
