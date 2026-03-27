use recallable::Recallable;

#[derive(Recallable)]
enum BorrowedEnum<'a> {
    Borrowed(&'a str),
}

fn main() {}
