use recallable::recallable_model;

#[recallable_model]
enum InvalidModelSkippedEnum {
    Ready {
        #[recallable(skip)]
        sticky: u8,
        value: u8,
    },
}

fn main() {}
