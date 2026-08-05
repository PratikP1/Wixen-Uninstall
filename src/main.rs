#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Wixen Uninstaller — entry point.
//!
//! Author: PratikP1
//!
//! Three ways in.  Normally the user launches Wixen, picks a product, and the
//! full escalating removal runs — preferably re-launched as `NT AUTHORITY\
//! SYSTEM`, headless, through `--execute`, with the interactive process showing
//! the UI and reading the results back.  When a removal queues a locked file for
//! boot-time deletion it registers a `RunOnce` relaunch with `--resume`; the
//! restart-launched run finishes the suspended cleanup and exits.  Neither the
//! `--execute` nor the `--resume` branch opens the menu, so a relaunch can never
//! loop back into a fresh removal.

use std::{io, process::ExitCode};
use wixen_uninstall_lib::{
    elevation::{NOT_ELEVATED_MESSAGE, is_elevated},
    executor::{LiveExecutor, finish_resume},
    forceful::LiveForcefulExecutor,
    plan::RemovalPlan,
    product::Product,
    reboot, system_exec,
    ui::{
        confirm_plan, run_full_removal, run_removal_via_system, select_product, show_error,
        show_report, show_restart_scheduled,
    },
    vendor::LiveVendorUninstaller,
};

/// Returned when Wixen refuses to start; nothing has been changed.
const NOT_ELEVATED_EXIT_CODE: u8 = 2;

fn main() -> io::Result<ExitCode> {
    if reboot::is_resume_request(std::env::args()) {
        return resume_suspended_removal();
    }

    if let Some(product) = system_exec::parse_execute_request(std::env::args()) {
        return execute_headless(product);
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

/// The headless branch a SYSTEM relaunch takes: run the removal, write the
/// results the interactive process reads back, and exit.
///
/// It shows no UI — a SYSTEM process has no desktop — and never opens the menu,
/// so a relaunch can never loop back into a fresh removal.
fn execute_headless(product: Product) -> io::Result<ExitCode> {
    // A SYSTEM token is elevated; if some misconfiguration launched this branch
    // unelevated it could only fail every action, so stop rather than write a
    // results file full of "access denied".
    if !is_elevated() {
        return Ok(ExitCode::from(NOT_ELEVATED_EXIT_CODE));
    }

    system_exec::run_and_write_results(product)?;
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

        // Prefer running the whole removal as SYSTEM — headless, past artifacts
        // ACL'd against Administrators.  The SYSTEM run registers any boot-time
        // resume itself and reports whether it did.  When that relaunch is
        // unavailable or fails, run the removal in-process under Administrator
        // and register the resume here instead.  Either way we end with the
        // report and whether Wixen will come back on its own after a restart.
        let (report, resume_registered) = match run_removal_via_system(&plan, product)? {
            Some(result) => result,
            None => {
                let (report, resume) = run_full_removal(
                    &plan,
                    &LiveVendorUninstaller,
                    &LiveExecutor,
                    &LiveForcefulExecutor,
                )?;
                let resume_registered = match &resume {
                    Some(state) => reboot::arrange_resume(state).is_ok(),
                    None => false,
                };
                (report, resume_registered)
            }
        };

        show_report(&report)?;
        if resume_registered {
            show_restart_scheduled()?;
        }

        return Ok(ExitCode::SUCCESS);
    }
}
