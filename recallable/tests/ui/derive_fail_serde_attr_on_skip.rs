use recallable::Recallable;

#[derive(Clone, serde::Serialize, Recallable)]
struct Foo {
    #[recallable(skip, rename = "x")]
    value: i32,
}

fn main() {}
