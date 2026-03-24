use recallable::recallable_model;

#[recallable_model]
struct ConflictingRecallableField {
    #[recallable]
    #[recallable(skip)]
    value: u32,
}

fn main() {}
