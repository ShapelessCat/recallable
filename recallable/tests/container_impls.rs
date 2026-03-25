#![cfg(feature = "serde")]

use recallable::{Recall, Recallable};
use serde::{Deserialize, de::DeserializeOwned};

#[derive(Clone, Debug, PartialEq, Deserialize, recallable::Recallable, recallable::Recall)]
struct Counter {
    value: i32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct ReplacingOption<T>(Option<T>);

impl<T> Recallable for ReplacingOption<T> {
    type Memento = Self;
}

impl<T> Recall for ReplacingOption<T> {
    fn recall(&mut self, memento: Self::Memento) {
        *self = memento;
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SelectiveOption<T>(Option<T>);

impl<T: Recallable> Recallable for SelectiveOption<T> {
    type Memento = Option<T::Memento>;
}

impl<T: Recall> Recall for SelectiveOption<T> {
    fn recall(&mut self, memento: Self::Memento) {
        if let (Some(current), Some(next)) = (&mut self.0, memento) {
            current.recall(next);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ZippedVec<T>(Vec<T>);

impl<T: Recallable> Recallable for ZippedVec<T> {
    type Memento = Vec<T::Memento>;
}

impl<T: Recall> Recall for ZippedVec<T> {
    fn recall(&mut self, memento: Self::Memento) {
        for (current, next) in self.0.iter_mut().zip(memento) {
            current.recall(next);
        }
    }
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct ReplacingOptionOuter {
    #[recallable]
    inner: ReplacingOption<Counter>,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct SelectiveOptionOuter {
    #[recallable]
    inner: SelectiveOption<Counter>,
}

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
struct ZippedVecOuter {
    #[recallable]
    inner: ZippedVec<Counter>,
}

trait IsReplacingOptionMemento {}

impl<T> IsReplacingOptionMemento for ReplacingOption<T> {}

trait IsOptionMemento {}

impl<T> IsOptionMemento for Option<T> {}

trait IsVecMemento {}

impl<T> IsVecMemento for Vec<T> {}

fn counter(value: i32) -> Counter {
    Counter { value }
}

fn counter_memento(value: i32) -> <Counter as Recallable>::Memento {
    parse_memento::<Counter>(&format!(r#"{{"value":{value}}}"#))
}

fn parse_memento<T>(json: &str) -> <T as Recallable>::Memento
where
    T: Recallable,
    <T as Recallable>::Memento: DeserializeOwned,
{
    serde_json::from_str(json).unwrap()
}

#[test]
fn derived_outers_accept_multiple_container_memento_shapes() {
    fn assert_is_replacing_option_memento<T: IsReplacingOptionMemento>() {}
    fn assert_is_option_memento<T: IsOptionMemento>() {}
    fn assert_is_vec_memento<T: IsVecMemento>() {}

    assert_is_replacing_option_memento::<<ReplacingOption<Counter> as Recallable>::Memento>();
    assert_is_option_memento::<<SelectiveOption<Counter> as Recallable>::Memento>();
    assert_is_vec_memento::<<ZippedVec<Counter> as Recallable>::Memento>();

    let _: fn(&mut ReplacingOptionOuter, <ReplacingOptionOuter as Recallable>::Memento) =
        <ReplacingOptionOuter as Recall>::recall;
    let _: fn(&mut SelectiveOptionOuter, <SelectiveOptionOuter as Recallable>::Memento) =
        <SelectiveOptionOuter as Recall>::recall;
    let _: fn(&mut ZippedVecOuter, <ZippedVecOuter as Recallable>::Memento) =
        <ZippedVecOuter as Recall>::recall;
}

#[test]
fn replacing_option_recall_replaces_the_whole_value() {
    let mut value = ReplacingOption(Some(counter(1)));
    value.recall(ReplacingOption(None));
    assert_eq!(value, ReplacingOption(None));
}

#[test]
fn selective_option_recall_updates_inner_value_when_both_sides_are_some() {
    let mut value = SelectiveOption(Some(counter(1)));
    value.recall(Some(counter_memento(9)));

    assert_eq!(value, SelectiveOption(Some(counter(9))));
}

#[test]
fn selective_option_recall_keeps_existing_some_when_memento_is_none() {
    let mut value = SelectiveOption(Some(counter(1)));
    value.recall(None);

    assert_eq!(value, SelectiveOption(Some(counter(1))));
}

#[test]
fn selective_option_recall_keeps_existing_none_when_memento_is_some() {
    let mut value = SelectiveOption::<Counter>(None);
    value.recall(Some(counter_memento(9)));

    assert_eq!(value, SelectiveOption(None));
}

#[test]
fn zipped_vec_recall_updates_only_the_shared_prefix_for_shorter_mementos() {
    let mut value = ZippedVec(vec![counter(1), counter(2), counter(3)]);
    value.recall(vec![counter_memento(10), counter_memento(20)]);

    assert_eq!(value, ZippedVec(vec![counter(10), counter(20), counter(3)]));
}

#[test]
fn zipped_vec_recall_ignores_extra_memento_elements_when_they_outnumber_values() {
    let mut value = ZippedVec(vec![counter(1), counter(2)]);
    value.recall(vec![
        counter_memento(10),
        counter_memento(20),
        counter_memento(30),
    ]);

    assert_eq!(value, ZippedVec(vec![counter(10), counter(20)]));
}
