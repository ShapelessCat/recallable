use recallable::{Recall, Recallable};

/// A type that deliberately does NOT implement Clone, Debug, or PartialEq.
struct Opaque(u8);

#[derive(Recallable, Recall)]
#[recallable(memento_derive_off)]
struct Holder {
    inner: Opaque,
}

fn main() {}
