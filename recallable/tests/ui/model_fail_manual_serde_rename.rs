use recallable::recallable_model;

#[recallable_model]
#[derive(Clone, Debug)]
struct Foo {
    #[serde(rename = "x")]
    value: i32,
}

fn main() {}
