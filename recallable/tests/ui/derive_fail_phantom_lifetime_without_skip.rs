use core::marker::PhantomData;

use recallable::{Recall, Recallable};

#[derive(Recallable, Recall)]
struct PhantomBorrow<'a> {
    marker: PhantomData<&'a ()>,
    value: u8,
}

fn main() {}
