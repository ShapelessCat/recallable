#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;

use crate::common::{FuzzOuterMemento, apply_memento};

fuzz_target!(|data: &[u8]| {
    // JSON must first survive UTF-8 validation; only then do we try memento
    // parsing and, on success, the recall path.
    if let Ok(payload) = core::str::from_utf8(data)
        && let Ok(memento) = serde_json::from_str::<FuzzOuterMemento>(payload)
    {
        apply_memento(memento);
    }
});
