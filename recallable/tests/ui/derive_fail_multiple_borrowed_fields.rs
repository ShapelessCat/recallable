use core::marker::PhantomData;
use recallable::Recallable;

#[derive(Recallable)]
struct MultiBorrowed<'a> {
    a: &'a str,
    b: Vec<&'a u8>,
    marker: PhantomData<&'a ()>,
}

fn main() {}
