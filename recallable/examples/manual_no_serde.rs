use recallable::{Recall, Recallable};

#[derive(Debug, PartialEq, Eq)]
struct EngineState {
    applied_ticks: u64,
    cached_checksum: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EngineMemento {
    applied_ticks: u64,
}

impl Recallable for EngineState {
    type Memento = EngineMemento;
}

impl Recall for EngineState {
    fn recall(&mut self, memento: Self::Memento) {
        self.applied_ticks = memento.applied_ticks;
    }
}

fn main() {
    let mut engine = EngineState {
        applied_ticks: 0,
        cached_checksum: 99,
    };

    engine.recall(EngineMemento { applied_ticks: 64 });

    assert_eq!(engine.applied_ticks, 64);
    assert_eq!(engine.cached_checksum, 99);
}
