use recallable::{Recall, Recallable};

#[derive(Recallable, Recall)]
enum InvalidSkippedRecallEnum {
    Ready {
        #[recallable(skip)]
        sticky: u8,
        value: u8,
    },
}

fn main() {}
