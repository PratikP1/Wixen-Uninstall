#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Wixen Uninstaller — entry point.
//!
//! Author: PratikP1

use std::io;
use wixen_uninstall_lib::{
    executor::{LiveExecutor, execute},
    plan::RemovalPlan,
    ui::{confirm_plan, select_product, show_report},
};

fn main() -> io::Result<()> {
    loop {
        let Some(product) = select_product()? else {
            #[cfg(not(target_os = "windows"))]
            println!("\nExiting. No changes were made.");
            return Ok(());
        };

        let plan = RemovalPlan::for_product(product);
        if !confirm_plan(&plan)? {
            continue;
        }

        let executor = LiveExecutor;
        let report = execute(&plan, &executor);
        show_report(&report)?;
        return Ok(());
    }
}
