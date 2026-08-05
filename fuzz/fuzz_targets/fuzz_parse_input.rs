//! Fuzz target: `menu::parse_input`.
//!
//! Author: PratikP1
//!
//! Invariants checked on every input:
//!   - The function never panics.
//!   - `MenuChoice::Product` is only returned for valid products.
//!   - `MenuChoice::Quit` is only returned for "q" / "Q".

#![no_main]
use libfuzzer_sys::fuzz_target;
use wixen_uninstall_lib::menu::{parse_input, MenuChoice};
use wixen_uninstall_lib::product::Product;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    match parse_input(s) {
        MenuChoice::Product(p) => {
            // Must be a real product known to the catalogue.
            assert!(Product::all().contains(&p));
        }
        MenuChoice::Quit => {
            // Must only result from "q" / "Q" (after trimming).
            let t = s.trim();
            assert!(
                t.eq_ignore_ascii_case("q"),
                "Quit should only come from 'q', got: {t:?}"
            );
        }
        MenuChoice::Invalid(_) => {
            // Anything else is fine — no further invariant to check.
        }
    }
});
