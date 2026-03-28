#![deny(warnings)]

use core::marker::PhantomData;

use recallable::{Recall, Recallable};

#[derive(Clone, Debug, PartialEq, recallable::Recallable, recallable::Recall)]
enum Example<T, const N: usize> {
    Idle,
    Loading(T),
    Ready {
        value: T,
        marker: PhantomData<[(); N]>,
    },
}

fn main() {
    type Memento = <Example<u8, 1> as Recallable>::Memento;

    let _ = Example::<u8, 1>::Idle;

    let mut state = Example::<u8, 1>::Loading(1);
    state.recall(Memento::Ready {
        value: 2,
    });

    let _ = state;
}
