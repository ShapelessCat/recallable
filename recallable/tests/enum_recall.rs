#![deny(dead_code)]

use core::marker::PhantomData;

use recallable::{Recall, Recallable};

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
enum AssignmentOnlyEnum<T, const N: usize> {
    #[allow(dead_code)]
    Idle,
    Loading(T),
    Ready {
        bytes: [u8; 2],
        version: u8,
        marker: PhantomData<[(); N]>,
    },
}

type AssignmentOnlyMemento = <AssignmentOnlyEnum<u32, 2> as Recallable>::Memento;

#[test]
fn test_assignment_only_enum_recall_switches_to_tuple_variant() {
    let mut state = AssignmentOnlyEnum::<u32, 2>::Idle;
    state.recall(AssignmentOnlyMemento::Loading(7));
    assert_eq!(state, AssignmentOnlyEnum::Loading(7));
}

#[test]
fn test_assignment_only_enum_recall_switches_to_named_variant() {
    let mut state = AssignmentOnlyEnum::<u32, 2>::Loading(1);
    state.recall(AssignmentOnlyMemento::Ready {
        bytes: [4, 5],
        version: 9,
        marker: PhantomData,
    });
    assert_eq!(
        state,
        AssignmentOnlyEnum::Ready {
            bytes: [4, 5],
            version: 9,
            marker: PhantomData,
        }
    );
}
