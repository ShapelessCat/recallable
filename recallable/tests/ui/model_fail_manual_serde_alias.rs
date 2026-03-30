use recallable::recallable_model;

#[recallable_model]
#[derive(Clone, Debug)]
struct Foo {
    #[serde(alias = "old")]
    value: i32,
}

fn main() {}
