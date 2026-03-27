use recallable::{Recall, Recallable};

#[derive(Clone, Debug, PartialEq, Recallable)]
struct Inner {
    value: u8,
}

#[derive(Recallable, Recall)]
enum InvalidNestedRecallEnum {
    Ready {
        #[recallable]
        inner: Inner,
    },
}

fn main() {}
