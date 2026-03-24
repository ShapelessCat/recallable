use recallable::Recallable;

#[derive(Recallable)]
struct InvalidNonPathRecallableField<T> {
    #[recallable]
    value: (T, T),
}

fn main() {}
