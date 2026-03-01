use recallable::Recallable;

#[derive(Recallable)]
struct InvalidRecallableNameValueParameter<T> {
    #[recallable = "skip"]
    value: T,
}

fn main() {}
