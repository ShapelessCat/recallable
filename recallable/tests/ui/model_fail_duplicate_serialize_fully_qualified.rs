use recallable::recallable_model;

#[recallable_model]
#[derive(::serde::Serialize)]
struct Foo {
    value: i32,
}

fn main() {}
