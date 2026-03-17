use recallable::recallable_model;
use serde::Serialize;

#[derive(Serialize)]
#[recallable_model]
struct Foo {
    value: i32,
}

fn main() {}
