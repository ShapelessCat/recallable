use recallable::Recallable;

#[derive(Recallable)]
struct ConflictingRecallableField {
    #[recallable]
    #[recallable(skip)]
    value: u32,
}

fn main() {}
