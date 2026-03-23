use recallable::Recallable;

#[derive(Recallable)]
struct BorrowedValue<'a> {
    value: &'a str,
}

fn main() {}
