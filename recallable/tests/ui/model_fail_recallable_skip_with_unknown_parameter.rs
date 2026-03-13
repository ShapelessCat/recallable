use recallable::recallable_model;

#[recallable_model]
struct BadSkip {
    #[recallable(skip, garbage)]
    value: i32,
    other: u32,
}

fn main() {}
