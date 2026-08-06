//! Fuzz target: `UninstallCommand::parse`.
//!
//! Author: PratikP1
//!
//! This parses a command line read from the registry: attacker-influenceable
//! input that then decides what elevated program Wixen runs.  The invariants:
//!   - Never panics, for any input, including multi-byte UTF-8 around quotes
//!     and braces.
//!   - A parsed command always has a non-empty program.
//!
//! The MSI-normalization guarantee (an `msiexec /x {code}` string is silent, and
//! an install `/i` becomes an uninstall `/x`) is covered by the unit tests in
//! `uninstall.rs`, which drive known inputs.  It is not asserted here: a bare
//! `msiexec.exe` typed into the registry parses to an ordinary, non-silent
//! program that also names msiexec, so no test on the output alone can tell a
//! normalized command from a passed-through one without false positives.  Safety
//! at run time is enforced separately, by only running a command that is silent.

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
});
