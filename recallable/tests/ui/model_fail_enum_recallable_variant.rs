use recallable::recallable_model;

#[derive(Clone, Debug, PartialEq)]
struct Inner {
    value: u8,
}

#[recallable_model]
enum InvalidModelNestedEnum {
    Ready {
        #[recallable]
        inner: Inner,
    },
}

fn main() {}
