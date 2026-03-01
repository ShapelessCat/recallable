use recallable::Recallable;

#[derive(Recallable)]
enum NotAStruct {
    Value(i32),
}

fn main() {}
