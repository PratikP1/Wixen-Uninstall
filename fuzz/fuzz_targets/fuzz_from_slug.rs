//! Fuzz target: `Product::from_slug`.
//!
//! Author: PratikP1
//!
//! Invariants:
//!   - Never panics.
//!   - Returned product (when `Some`) must be in `Product::all()`.

#![no_main]
use libfuzzer_sys::fuzz_target;
use wixen_uninstall_lib::product::Product;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    if let Some(p) = Product::from_slug(s) {
        assert!(Product::all().contains(&p));
    }
});
