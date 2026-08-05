//! Fuzz target: `WindowsLocations::resolve`.
//!
//! Author: PratikP1
//!
//! This is the parser that decides what Wixen is willing to delete
//! recursively, so its invariants are the ones worth hammering:
//!   - Never panics, for any input, including multi-byte UTF-8 around the
//!     `{` / `}` placeholder delimiters.
//!   - Anything it accepts is an absolute path with at least two segments
//!     below the drive, and contains no `.` or `..` segment.
//!   - Nothing it accepts is a protected system directory.

#![no_main]
use libfuzzer_sys::fuzz_target;
use wixen_uninstall_lib::paths::WindowsLocations;

const NEVER_DELETABLE: &[&str] = &[
    r"C:",
    r"C:\Windows",
    r"C:\Windows\System32",
    r"C:\Windows\System32\drivers",
    r"C:\Program Files",
    r"C:\Program Files (x86)",
    r"C:\Program Files\Common Files",
    r"C:\ProgramData",
    r"C:\Users",
];

fuzz_target!(|data: &[u8]| {
    let Ok(template) = std::str::from_utf8(data) else {
        return;
    };

    let locations = WindowsLocations::conventional("C:");
    let Ok(resolved) = locations.resolve(template) else {
        return;
    };

    let (drive, below_drive) = resolved
        .split_once('\\')
        .expect("an accepted path always has a drive prefix");
    assert!(drive.ends_with(':'), "unexpected prefix in {resolved:?}");

    let segments: Vec<&str> = below_drive.trim_end_matches('\\').split('\\').collect();
    assert!(
        segments.len() >= 2,
        "{resolved:?} is too close to the drive root"
    );
    for segment in &segments {
        assert!(
            !segment.is_empty() && *segment != "." && *segment != "..",
            "{resolved:?} contains a relative or empty segment"
        );
    }

    let target = resolved.trim_end_matches('\\');
    for protected in NEVER_DELETABLE {
        assert!(
            !protected.eq_ignore_ascii_case(target),
            "resolver accepted the protected location {resolved:?}"
        );
    }
});
