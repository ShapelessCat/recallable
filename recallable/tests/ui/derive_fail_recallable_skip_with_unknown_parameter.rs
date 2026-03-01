use recallable::Recallable;

#[derive(Recallable)]
struct InvalidRecallableSkipParameter<T> {
    #[recallable(skip, unknown)]
    value: T,
}

fn main() {}
