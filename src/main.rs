#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Wixen Uninstaller — entry point.
//!
//! Author: PratikP1
//!
//! Two ways in.  Normally the user launches Wixen, picks a product, and the
//! full escalating removal runs.  When that removal has to queue a locked file
//! for boot-time deletion it registers a `RunOnce` relaunch with `--resume`;
//! the restart-launched run takes the `--resume` branch, finishes the suspended
//! cleanup, and exits.  Because that branch never opens the menu, a relaunch
//! can never loop back into a fresh removal.

use std::{io, process::ExitCode};
use wixen_uninstall_lib::{
    elevation::{NOT_ELEVATED_MESSAGE, is_elevated},
    executor::{LiveExecutor, finish_resume},
    forceful::LiveForcefulExecutor,
    plan::RemovalPlan,
    reboot,
    ui::{
        confirm_plan, run_full_removal, select_product, show_error, show_report,
        show_restart_scheduled,
    },
    vendor::LiveVendorUninstaller,
};

/// Returned when Wixen refuses to start; nothing has been changed.
const NOT_ELEVATED_EXIT_CODE: u8 = 2;

fn main() -> io::Result<ExitCode> {
    if reboot::is_resume_request(std::env::args()) {
        return resume_suspended_removal();
    }

    // The shipped executable carries a manifest that makes Windows elevate it.
    // If that ever fails, stop here rather than failing every single action and
    // burying the real cause under dozens of "Access is denied" messages.
    if !is_elevated() {
        show_error(NOT_ELEVATED_MESSAGE)?;
        return Ok(ExitCode::from(NOT_ELEVATED_EXIT_CODE));
    }

    normal_removal()
}

/// Finish a removal that queued files for boot-time deletion before a restart.
///
/// Elevation is checked *before* the state is consumed: a non-elevated resume
/// would delete the state it has no privilege to act on, stranding the cleanup
/// with no record of what was left to do.
fn resume_suspended_removal() -> io::Result<ExitCode> {
    if !is_elevated() {
        show_error(NOT_ELEVATED_MESSAGE)?;
        return Ok(ExitCode::from(NOT_ELEVATED_EXIT_CODE));
    }

    let Some(state) = reboot::take_pending_resume() else {
        // Nothing to finish — the state was already consumed, or never written.
        return Ok(ExitCode::SUCCESS);
    };

    let report = finish_resume(&state, &LiveExecutor);
    show_report(&report)?;
    Ok(ExitCode::SUCCESS)
}

/// The ordinary interactive flow: choose a product, confirm, remove.
fn normal_removal() -> io::Result<ExitCode> {
    loop {
        let Some(product) = select_product()? else {
            #[cfg(not(target_os = "windows"))]
            println!("\nExiting. No changes were made.");
            return Ok(ExitCode::SUCCESS);
        };

        let plan = RemovalPlan::for_product(product);
        if !confirm_plan(&plan)? {
            continue;
        }

        let (report, resume) = run_full_removal(
            &plan,
            &LiveVendorUninstaller,
            &LiveExecutor,
            &LiveForcefulExecutor,
        )?;

        // When files were queued for boot-time deletion, register the resume
        // before promising anything: only a resume we have actually arranged
        // lets us tell the user Wixen will come back on its own.  If that
        // registration fails we fall back to the report alone, whose footer
        // still tells them to run Wixen again after restarting.
        let resume_registered = match &resume {
            Some(state) => reboot::arrange_resume(state).is_ok(),
            None => false,
        };

        show_report(&report)?;
        if resume_registered {
            show_restart_scheduled()?;
        }

        return Ok(ExitCode::SUCCESS);
    }
}
