use recallable::{Recall, Recallable};

/// A type that deliberately does NOT implement Clone, Debug, or PartialEq.
/// It does implement Deserialize because memento_derive_off preserves serde derives.
#[derive(serde::Deserialize)]
struct Opaque(u8);

#[derive(Recallable, Recall)]
#[recallable(memento_derive_off)]
struct Holder {
    inner: Opaque,
}

fn main() {}
