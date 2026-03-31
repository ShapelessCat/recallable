use recallable::Recallable;

#[derive(Clone, serde::Serialize, Recallable)]
struct Foo {
    #[serde(rename = "x")]
    #[recallable(rename = "y")]
    value: i32,
}

fn main() {}
