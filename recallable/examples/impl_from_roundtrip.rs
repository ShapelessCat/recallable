use core::marker::PhantomData;

use recallable::{Recall, Recallable, recallable_model};

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct InnerState {
    value: i32,
}

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct DerivedEnvelope<T> {
    #[recallable]
    inner: T,
    version: u32,
    #[recallable(skip)]
    marker: PhantomData<T>,
}

fn main() {
    let original = DerivedEnvelope {
        inner: InnerState { value: 42 },
        version: 7,
        marker: PhantomData::<InnerState>,
    };

    let memento: <DerivedEnvelope<InnerState> as Recallable>::Memento = original.clone().into();

    let mut target = DerivedEnvelope {
        inner: InnerState { value: 0 },
        version: 0,
        marker: PhantomData::<InnerState>,
    };

    target.recall(memento);

    assert_eq!(target, original);
}
