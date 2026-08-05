//! Fuzz target: `UninstallCommand::parse`.
//!
//! Author: PratikP1
//!
//! This parses a command line read from the registry — attacker-influenceable
//! input that then decides what elevated program Wixen runs.  The invariants:
//!   - Never panics, for any input, including multi-byte UTF-8 around quotes
//!     and braces.
//!   - A parsed command always has a non-empty program.
//!   - An MSI command is always the silent `msiexec /x {code}` form.

#![no_main]
use libfuzzer_sys::fuzz_target;
use wixen_uninstall_lib::uninstall::UninstallCommand;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let Ok(command) = UninstallCommand::parse(text) else {
        return;
    };

    assert!(
        !command.program.is_empty(),
        "an accepted command must name a program to run: {text:?}"
    );

    // The only way parse yields msiexec is the MSI normalization, which must be
    // a silent uninstall — never an install, never interactive.
    if command.program.eq_ignore_ascii_case("msiexec.exe") {
        assert_eq!(command.args.first().map(String::as_str), Some("/x"));
        assert!(
            command.is_silent(),
            "a normalized MSI command must be unattended: {command:?}"
        );
    }
});
