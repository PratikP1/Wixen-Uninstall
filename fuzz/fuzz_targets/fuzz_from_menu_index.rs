//! Fuzz target: `Product::from_menu_index`.
//!
//! Author: PratikP1
//!
//! Invariants:
//!   - Never panics for any `usize` value.
//!   - Index must be in 1..=len for `Some` to be returned.

#![no_main]
use libfuzzer_sys::fuzz_target;
use wixen_uninstall_lib::product::Product;

fuzz_target!(|data: &[u8]| {
    // Interpret the first 8 bytes as a little-endian usize.
    let mut buf = [0u8; 8];
    let len = data.len().min(8);
    buf[..len].copy_from_slice(&data[..len]);
    let index = usize::from_le_bytes(buf);

    let result = Product::from_menu_index(index);
    if let Some(p) = result {
        assert!(Product::all().contains(&p));
        // 1-based index must be within catalogue length.
        assert!(index >= 1 && index <= Product::all().len());
    }
});
