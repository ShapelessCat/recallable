use recallable::{Recall, Recallable};

/// A type that deliberately does NOT implement Clone, Debug, or PartialEq.
/// It does implement Deserialize because skip_memento_default_derives preserves serde derives.
#[derive(serde::Deserialize)]
struct Opaque(u8);

#[derive(Recallable, Recall)]
#[recallable(skip_memento_default_derives)]
struct Holder {
    inner: Opaque,
}

fn main() {}
