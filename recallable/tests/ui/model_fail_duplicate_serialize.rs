use recallable::recallable_model;
use serde::Serialize;

#[recallable_model]
#[derive(Serialize)]
struct Foo {
    value: i32,
}

fn main() {}
