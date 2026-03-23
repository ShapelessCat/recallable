use recallable::Recallable;

#[derive(Recallable)]
struct LifetimeParameterized<'a> {
    marker: core::marker::PhantomData<&'a ()>,
    value: u8,
}

fn main() {}
