use recallable::Recallable;

#[derive(Recallable)]
struct InvalidRecallableParameter<T> {
    #[recallable(unknown)]
    value: T,
}

fn main() {}
