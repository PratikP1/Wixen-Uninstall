#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Wixen Uninstaller — entry point.
//!
//! Author: PratikP1

use std::{io, process::ExitCode};
use wixen_uninstall_lib::{
    elevation::{NOT_ELEVATED_MESSAGE, is_elevated},
    executor::LiveExecutor,
    plan::RemovalPlan,
    ui::{confirm_plan, run_removal, select_product, show_error, show_report},
};

/// Returned when Wixen refuses to start; nothing has been changed.
const NOT_ELEVATED_EXIT_CODE: u8 = 2;

fn main() -> io::Result<ExitCode> {
    // The shipped executable carries a manifest that makes Windows elevate it.
    // If that ever fails, stop here rather than failing every single action and
    // burying the real cause under dozens of "Access is denied" messages.
    if !is_elevated() {
        show_error(NOT_ELEVATED_MESSAGE)?;
        return Ok(ExitCode::from(NOT_ELEVATED_EXIT_CODE));
    }

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

        let report = run_removal(&plan, &LiveExecutor)?;
        show_report(&report)?;
        return Ok(ExitCode::SUCCESS);
    }
}
