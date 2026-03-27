use recallable::Recallable;

#[derive(Recallable)]
union NotSupported {
    value: u32,
}

fn main() {}
