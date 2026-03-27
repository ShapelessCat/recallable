#![cfg(feature = "serde")]

use recallable::{Recall, Recallable, recallable_model};

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
enum ModelEnum {
    Idle,
    Ready { value: u32, label: String },
}

#[test]
fn test_recallable_model_supports_assignment_only_enums() {
    let expected = ModelEnum::Ready {
        value: 7,
        label: "ok".to_string(),
    };
    let json = serde_json::to_string(&expected).unwrap();
    let memento: <ModelEnum as Recallable>::Memento = serde_json::from_str(&json).unwrap();

    let mut actual = ModelEnum::Idle;
    actual.recall(memento);

    assert_eq!(actual, expected);
}
