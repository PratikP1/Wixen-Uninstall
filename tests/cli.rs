//! End-to-end tests that drive the real binary.
//!
//! Author: PratikP1
//!
//! Everything else in the suite exercises the library.  These run the compiled
//! executable with piped stdin and stdout, which is the only way to cover
//! `main` and the platform dispatch in `ui`: those functions read the real
//! standard streams, so nothing short of a child process can observe them.
//!
//! Only meaningful off Windows.  There the binary is a `windows` subsystem
//! application that opens dialogs and never touches stdio, so these are
//! compiled out rather than left to hang a CI runner.
#![cfg(not(target_os = "windows"))]

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

/// Runs the binary with `args` and `input` on stdin and returns what it produced.
///
/// `LiveExecutor` is a no-op off Windows, so a full run touches nothing on
/// the machine running the tests.
fn run_with_args(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wixen_uninstall"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary should start");

    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(input.as_bytes())
        .expect("the child should accept input");

    child.wait_with_output().expect("the binary should exit")
}

/// Runs the binary with `input` on stdin and no arguments.
fn run_with_input(input: &str) -> Output {
    run_with_args(&[], input)
}

fn stdout_of(input: &str) -> String {
    let output = run_with_input(input);
    assert!(
        output.status.success(),
        "exited with {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("output should be UTF-8")
}

// ─── Quitting ────────────────────────────────────────────────────────────────

#[test]
fn quitting_changes_nothing_and_reports_so() {
    let stdout = stdout_of("q\n");

    assert!(stdout.contains("select a product to remove"));
    assert!(stdout.contains("No changes were made"));
    assert!(
        !stdout.contains("Report"),
        "quitting must not run a removal: {stdout}"
    );
}

#[test]
fn end_of_input_is_treated_as_quitting() {
    let stdout = stdout_of("");
    assert!(stdout.contains("No changes were made"));
}

// ─── A full run ──────────────────────────────────────────────────────────────

#[test]
fn selecting_a_product_runs_the_whole_pipeline() {
    let stdout = stdout_of("1\n");

    // Menu, then confirmation, then the plan, then progress, then the report:
    // every stage has to appear, in order.
    let stages = [
        "select a product to remove",
        "Remove McAfee Total Protection?",
        "Folders to delete",
        "Removing scheduled tasks",
        "Cleaning up registry keys",
        "Report",
        "McAfee Total Protection was removed.",
        "Restart Windows to finish the cleanup.",
    ];

    let mut searched_from = 0;
    for stage in stages {
        let found = stdout[searched_from..].find(stage).unwrap_or_else(|| {
            panic!("missing stage {stage:?} after position {searched_from} in:\n{stdout}")
        });
        searched_from += found + stage.len();
    }
}

#[test]
fn the_confirmation_lists_what_will_be_removed_before_any_progress() {
    let stdout = stdout_of("3\n");

    let listing = stdout
        .find("Driver files to delete")
        .expect("the plan should be shown");
    let first_progress = stdout
        .find("Removing scheduled tasks")
        .expect("progress should be reported");

    assert!(
        listing < first_progress,
        "the user must see the plan before anything runs:\n{stdout}"
    );
    assert!(stdout.contains("aswSP.sys"), "real paths should be listed");
}

#[test]
fn every_product_can_be_selected_by_number() {
    for (choice, expected) in [
        ("1", "McAfee Total Protection"),
        ("2", "Norton 360 / Norton Security"),
        ("3", "Avast Antivirus / Avast Premium Security"),
        ("4", "AVG AntiVirus / AVG Internet Security"),
    ] {
        let stdout = stdout_of(&format!("{choice}\n"));
        assert!(
            stdout.contains(&format!("{expected} was removed.")),
            "choice {choice} should remove {expected}:\n{stdout}"
        );
    }
}

#[test]
fn a_product_can_be_selected_by_slug() {
    let stdout = stdout_of("norton\n");
    assert!(stdout.contains("Norton 360 / Norton Security was removed."));
}

// ─── Resume after a restart ────────────────────────────────────────────────────

#[test]
fn resume_mode_with_nothing_pending_exits_quietly() {
    // This is the branch a RunOnce relaunch takes after a restart. Off Windows
    // there is never any saved state to finish, so the run must exit cleanly,
    // show no report, and — critically — never fall through to the product menu.
    // A restart-launched Wixen must not offer to start a fresh removal.
    let output = run_with_args(&["--resume"], "");

    assert!(
        output.status.success(),
        "resume must exit cleanly, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("output should be UTF-8");
    assert!(
        !stdout.contains("Report"),
        "with nothing pending there is nothing to report: {stdout}"
    );
    assert!(
        !stdout.contains("select a product to remove"),
        "a resume run must never open the menu: {stdout}"
    );
}

// ─── Headless SYSTEM execution ─────────────────────────────────────────────────

#[test]
fn execute_mode_with_a_product_exits_quietly() {
    // This is the headless branch a SYSTEM relaunch takes. Off Windows the Live
    // executors are no-ops, so it does its (empty) work, writes its report to a
    // file rather than the console, and exits cleanly. It must never open the
    // product menu: a SYSTEM relaunch must not start a fresh interactive removal.
    let output = run_with_args(&["--execute", "avast"], "");

    assert!(
        output.status.success(),
        "headless execute must exit cleanly, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("output should be UTF-8");
    assert!(
        !stdout.contains("select a product to remove"),
        "a headless SYSTEM run must never open the menu: {stdout}"
    );
    assert!(
        !stdout.contains("Report"),
        "the headless run reports to a file, not to the console: {stdout}"
    );
}

// ─── Invalid input ───────────────────────────────────────────────────────────

#[test]
fn an_invalid_choice_reprompts_rather_than_exiting() {
    let stdout = stdout_of("banana\n2\n");

    assert!(stdout.contains("Invalid choice"));
    assert!(stdout.contains("Norton 360 / Norton Security was removed."));
}

// ─── Accessibility ───────────────────────────────────────────────────────────

#[test]
fn no_stage_of_a_full_run_emits_ansi_escapes() {
    // Screen readers read the terminal buffer; escape sequences end up spoken.
    let stdout = stdout_of("4\n");
    assert!(
        !stdout.contains('\x1b'),
        "output must stay free of ANSI escapes"
    );
}
