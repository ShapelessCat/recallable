use recallable::{Recall, Recallable, recallable_model};

const CAPACITY: usize = 128;

#[recallable_model]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SensorReading {
    id: u16,
    value: i32,
}

fn main() {
    let original = SensorReading { id: 7, value: 42 };
    let bytes = postcard::to_vec::<_, CAPACITY>(&original).unwrap();
    let memento: <SensorReading as Recallable>::Memento = postcard::from_bytes(&bytes).unwrap();

    let mut restored = SensorReading::default();
    restored.recall(memento);

    assert_eq!(restored, original);
}
