#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;

use crate::common::{FuzzOuterMemento, apply_memento};

fuzz_target!(|data: &[u8]| {
    // Postcard consumes raw bytes directly, so any accepted decode is
    // immediately pushed through recall to look for panic-inducing states.
    if let Ok(memento) = postcard::from_bytes::<FuzzOuterMemento>(data) {
        apply_memento(memento);
    }
});
