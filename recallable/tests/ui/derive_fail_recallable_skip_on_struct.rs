use recallable::Recallable;

#[derive(Recallable)]
#[recallable(skip)]
struct Example {
    value: u32,
}

fn main() {}
