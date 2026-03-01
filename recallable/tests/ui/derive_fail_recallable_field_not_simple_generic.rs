use recallable::Recallable;

#[derive(Recallable)]
struct InvalidNestedRecallableType<T> {
    #[recallable]
    value: Option<T>,
}

fn main() {}
