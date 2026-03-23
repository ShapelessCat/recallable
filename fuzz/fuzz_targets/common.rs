use recallable::{Recall, Recallable, recallable_model};

// Keep the fuzz model intentionally small while still covering the two
// behaviors most likely to regress in generated code: nested recall and
// skipped-field preservation.
#[recallable_model]
#[derive(Clone, Debug)]
pub struct FuzzInner {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}

#[recallable_model]
#[derive(Clone, Debug)]
pub struct FuzzOuter {
    pub level: u8,
    #[recallable]
    pub nested: FuzzInner,
    #[allow(dead_code)]
    #[recallable(skip)]
    pub skipped: u8,
}

pub type FuzzOuterMemento = <FuzzOuter as Recallable>::Memento;

pub fn seed_model() -> FuzzOuter {
    // Start from a stable target so every successful deserialize also exercises
    // recall against an existing instance rather than constructing from scratch.
    FuzzOuter {
        level: 0,
        nested: FuzzInner {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        },
        skipped: 99,
    }
}

pub fn apply_memento(memento: FuzzOuterMemento) {
    // The fuzz target is interested in panic safety of the full
    // deserialize-and-recall path, not deserialization alone.
    let mut target = seed_model();
    target.recall(memento);
}
