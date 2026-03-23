use std::fmt;
use std::string::String;
use std::string::ToString;

use recallable::{Recallable, TryRecall};

#[derive(Debug)]
struct FallibleStruct {
    value: i32,
}

#[derive(Debug, Clone)]
struct FallibleStructMemento(i32);

#[derive(Debug)]
struct RecallError(String);

impl fmt::Display for RecallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecallError: {}", self.0)
    }
}

impl core::error::Error for RecallError {}

impl Recallable for FallibleStruct {
    type Memento = FallibleStructMemento;
}

impl From<FallibleStruct> for FallibleStructMemento {
    fn from(s: FallibleStruct) -> Self {
        FallibleStructMemento(s.value)
    }
}

impl TryRecall for FallibleStruct {
    type Error = RecallError;

    fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error> {
        if memento.0 < 0 {
            return Err(RecallError("Value cannot be negative".to_string()));
        }
        self.value = memento.0;
        Ok(())
    }
}

#[test]
fn test_try_recall_custom_error() {
    let mut s = FallibleStruct { value: 0 };

    // Valid recall
    assert!(s.try_recall(FallibleStructMemento(10)).is_ok());
    assert_eq!(s.value, 10);

    // Invalid recall
    let result = s.try_recall(FallibleStructMemento(-5));
    assert!(result.is_err());
    assert_eq!(s.value, 10); // Should not have changed

    match result {
        Err(e) => assert_eq!(e.to_string(), "RecallError: Value cannot be negative"),
        _ => panic!("Expected error"),
    }
}

mod phantom_lifetime {
    use core::marker::PhantomData;
    use recallable::{Recall, Recallable};

    #[derive(Recallable, Recall)]
    struct PhantomLifetime<'a> {
        marker: PhantomData<&'a ()>,
        value: u8,
    }

    type PhantomLifetimeMemento = <PhantomLifetime<'static> as Recallable>::Memento;

    #[test]
    fn test_phantom_lifetime_recall() {
        let mut s = PhantomLifetime {
            marker: PhantomData,
            value: 10,
        };
        // marker is auto-skipped from memento (PhantomData with struct lifetime)
        let memento = PhantomLifetimeMemento { value: 42 };
        s.recall(memento);
        assert_eq!(s.value, 42);
    }
}

mod skipped_borrowed_field {
    use recallable::{Recall, Recallable};

    #[derive(Recallable, Recall)]
    struct WithSkippedBorrow<'a> {
        #[recallable(skip)]
        name: &'a str,
        value: u8,
    }

    type WithSkippedBorrowMemento = <WithSkippedBorrow<'static> as Recallable>::Memento;

    #[test]
    fn test_skipped_borrowed_field_recall() {
        let mut s = WithSkippedBorrow {
            name: "hello",
            value: 10,
        };
        let memento = WithSkippedBorrowMemento { value: 42 };
        s.recall(memento);
        assert_eq!(s.value, 42);
        assert_eq!(s.name, "hello"); // unchanged — skipped
    }
}
