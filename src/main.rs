//! Wixen Uninstaller — entry point.
//!
//! Author: PratikP1

use std::io;
use wixen_uninstall_lib::{
    executor::{LiveExecutor, execute},
    menu::run_menu,
    plan::RemovalPlan,
};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut input = stdin.lock();
    let mut output = stdout.lock();

    match run_menu(&mut input, &mut output)? {
        None => {
            println!("\nExiting. No changes were made.");
        }
        Some(product) => {
            println!("\nBuilding removal plan for {product}…");
            let plan = RemovalPlan::for_product(product);
            println!(
                "Plan contains {} action(s). Starting removal (Administrator privileges required)…\n",
                plan.action_count()
            );

            let executor = LiveExecutor;
            let report = execute(&plan, &executor);

            println!("─── Report ───────────────────────────────────────────────");
            println!("Product       : {}", report.product_name);
            println!("Actions tried : {}", report.actions_attempted);
            println!("Succeeded     : {}", report.actions_succeeded);
            if report.fully_succeeded() {
                println!("Status        : SUCCESS — all artifacts removed.");
            } else {
                println!(
                    "Status        : PARTIAL — {} error(s):",
                    report.errors.len()
                );
                for err in &report.errors {
                    println!("  • {err}");
                }
            }
        }
    }

    Ok(())
}
