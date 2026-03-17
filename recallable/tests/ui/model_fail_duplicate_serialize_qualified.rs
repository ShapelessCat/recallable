use recallable::recallable_model;

#[derive(serde::Serialize)]
#[recallable_model]
struct Foo {
    value: i32,
}

fn main() {}
