//! User-interface entry points and the text every screen shows.
//!
//! Author: PratikP1
//!
//! On Windows the binary drives native Win32 **task dialogs** — the modern
//! dialog Windows itself uses, with a main instruction, command links, an
//! expandable details pane, and a progress bar.  Every control is a real
//! system control, which is what makes the app work with NVDA, JAWS, and
//! Narrator without any bespoke accessibility code: each product is its own
//! labelled button rather than a "Yes"/"No" whose meaning lives in the body
//! text.
//!
//! The wording of every screen is built here, by plain functions over plain
//! data, so it can be unit tested on any platform.  Only the drawing lives
//! behind `#[cfg(target_os = "windows")]`.

#[cfg(target_os = "windows")]
mod task_dialog;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
use crate::executor::execute_with_progress;
#[cfg(not(target_os = "windows"))]
use crate::menu::run_menu;
use crate::{
    executor::{ExecutionReport, Executor, RemovalPhase},
    plan::RemovalPlan,
    product::Product,
};
use std::io;

pub const APP_TITLE: &str = "Wixen Uninstaller";
pub const HELP_FILE_NAME: &str = "WixenUninstallerHelp.html";

// ─── Public API ──────────────────────────────────────────────────────────────

/// Select the product to remove.  `None` means the user chose to quit.
pub fn select_product() -> io::Result<Option<Product>> {
    #[cfg(target_os = "windows")]
    {
        windows::select_product()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut input = stdin.lock();
        let mut output = stdout.lock();
        run_menu(&mut input, &mut output)
    }
}

/// Show what will be removed and ask for confirmation.
pub fn confirm_plan(plan: &RemovalPlan) -> io::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows::confirm_plan(plan)
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("\n{}", confirmation_heading(plan));
        println!("{}\n", confirmation_body(plan));
        println!("{}", plan_details(plan));
        Ok(true)
    }
}

/// Run the plan, showing progress while it works.
pub fn run_removal(
    plan: &RemovalPlan,
    executor: &(dyn Executor + Sync),
) -> io::Result<ExecutionReport> {
    #[cfg(target_os = "windows")]
    {
        windows::run_removal(plan, executor)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut last_phase = None;
        Ok(execute_with_progress(plan, executor, &mut |phase, done| {
            if last_phase != Some(phase) {
                last_phase = Some(phase);
                println!("{}", phase.description());
            }
            let _ = done;
        }))
    }
}

/// Present a blocking error.  Nothing has been changed.
pub fn show_error(message: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::show_error(message)
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("{message}");
        Ok(())
    }
}

/// Present the final execution report.
pub fn show_report(report: &ExecutionReport) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::show_report(report)
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("─── Report ───────────────────────────────────────────────");
        println!("{}", report_heading(report));
        println!("{}", report_body(report));
        Ok(())
    }
}

// ─── Screen text ─────────────────────────────────────────────────────────────

/// Removing kernel drivers and services only takes full effect after a restart.
pub const RESTART_ADVICE: &str = "Restart Windows to finish the cleanup.";

/// Shown in every dialog's footer.
pub const HELP_FOOTER: &str = "Press F1 for help.";

/// Longest details pane we will build, per category.  A task dialog does not
/// scroll its expanded area, so an uncapped list would grow the window past the
/// bottom of the screen.
const MAX_DETAIL_LINES_PER_CATEGORY: usize = 8;

/// Main instruction on the product-selection screen.
pub fn selection_heading() -> &'static str {
    "Which product do you want to remove?"
}

/// Body text on the product-selection screen.
pub fn selection_body() -> &'static str {
    "Wixen removes the program along with the services, scheduled tasks, files, \
     and registry keys it leaves behind. Choose a product to see exactly what \
     will be removed before anything is deleted."
}

/// Main instruction on the confirmation screen.
pub fn confirmation_heading(plan: &RemovalPlan) -> String {
    format!("Remove {}?", plan.product.display_name())
}

/// Body text on the confirmation screen.
pub fn confirmation_body(plan: &RemovalPlan) -> String {
    let mut body = format!(
        "Wixen will attempt {} actions. This cannot be undone, and you should \
         restart Windows afterwards.",
        plan.action_count()
    );

    if let Some(note) = plan.product.pre_removal_note() {
        body.push_str("\n\n");
        body.push_str(note);
    }

    body
}

/// The expandable "what will be removed" pane.
pub fn plan_details(plan: &RemovalPlan) -> String {
    let directories: Vec<&str> = plan
        .file_paths
        .iter()
        .filter(|file| file.is_dir)
        .map(|file| file.path.as_str())
        .collect();
    let drivers: Vec<&str> = plan
        .file_paths
        .iter()
        .filter(|file| !file.is_dir)
        .map(|file| file.path.as_str())
        .collect();
    let services: Vec<&str> = plan
        .services
        .iter()
        .map(|service| service.name.as_str())
        .collect();
    let tasks: Vec<&str> = plan
        .scheduled_tasks
        .iter()
        .map(|task| task.task_path.as_str())
        .collect();
    let registry: Vec<&str> = plan
        .registry_entries
        .iter()
        .map(|entry| entry.key_path.as_str())
        .collect();

    [
        detail_section("Folders to delete", &directories),
        detail_section("Driver files to delete", &drivers),
        detail_section("Services to stop and remove", &services),
        detail_section("Scheduled tasks to delete", &tasks),
        detail_section("Registry keys to delete", &registry),
    ]
    .join("\n\n")
}

fn detail_section(heading: &str, entries: &[&str]) -> String {
    let mut section = format!("{heading} ({}):", entries.len());

    for entry in entries.iter().take(MAX_DETAIL_LINES_PER_CATEGORY) {
        section.push_str("\n    ");
        section.push_str(entry);
    }

    if let Some(remaining) = entries.len().checked_sub(MAX_DETAIL_LINES_PER_CATEGORY)
        && remaining > 0
    {
        section.push_str(&format!("\n    …and {remaining} more"));
    }

    section
}

/// Main instruction while the removal runs.
pub fn progress_heading(plan: &RemovalPlan) -> String {
    format!("Removing {}…", plan.product.display_name())
}

/// Body text while the removal runs.
pub fn progress_body(phase: RemovalPhase, completed: usize, total: usize) -> String {
    format!("{}  ({completed} of {total})", phase.description())
}

/// Main instruction on the report screen.
pub fn report_heading(report: &ExecutionReport) -> String {
    if report.fully_succeeded() {
        format!("{} was removed.", report.product_name)
    } else if report.actions_succeeded == 0 {
        format!("{} could not be removed.", report.product_name)
    } else {
        format!("{} was partly removed.", report.product_name)
    }
}

/// Body text on the report screen.
pub fn report_body(report: &ExecutionReport) -> String {
    let mut body = format!(
        "{} of {} actions succeeded.",
        report.actions_succeeded, report.actions_attempted
    );

    if !report.errors.is_empty() {
        body.push_str(&format!("\n{} failed.", report.errors.len()));
    }
    if report.actions_skipped() > 0 {
        body.push_str(&format!(
            "\n{} were skipped to keep Windows bootable.",
            report.actions_skipped()
        ));
    }

    body.push_str("\n\n");
    body.push_str(RESTART_ADVICE);
    body
}

/// The expandable pane on the report screen: what went wrong, and why.
pub fn report_details(report: &ExecutionReport) -> Option<String> {
    if report.fully_succeeded() {
        return None;
    }

    let mut details = String::new();
    if !report.errors.is_empty() {
        details.push_str("Failed:");
        for error in &report.errors {
            details.push_str(&format!("\n    {error}"));
        }
    }

    if !report.warnings.is_empty() {
        if !details.is_empty() {
            details.push_str("\n\n");
        }
        details.push_str("Skipped for safety:");
        for warning in &report.warnings {
            details.push_str(&format!("\n    {warning}"));
        }
    }

    Some(details)
}

/// Footer on the report screen: the single most useful next step, or nothing.
pub fn report_footer(report: &ExecutionReport) -> Option<&'static str> {
    if report.fully_succeeded() {
        return None;
    }

    Some(
        "Some parts of this product defend themselves while Windows is running \
         normally. Reboot into Windows Safe Mode and run Wixen again to finish.",
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{executor::ExecutionReport, plan::RemovalPlan, product::Product};

    fn report(succeeded: usize, warnings: Vec<String>, errors: Vec<String>) -> ExecutionReport {
        ExecutionReport {
            product_name: Product::Avast.display_name().to_owned(),
            actions_attempted: 10,
            actions_succeeded: succeeded,
            warnings,
            errors,
        }
    }

    // ── Selection ────────────────────────────────────────────────────────────

    #[test]
    fn selection_screen_asks_a_direct_question() {
        assert!(selection_heading().ends_with('?'));
        assert!(selection_body().contains("before anything is deleted"));
    }

    // ── Confirmation ─────────────────────────────────────────────────────────

    #[test]
    fn confirmation_names_the_product_and_action_count() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        let heading = confirmation_heading(&plan);
        let body = confirmation_body(&plan);

        assert!(heading.contains(Product::McAfee.display_name()));
        assert!(body.contains(&plan.action_count().to_string()));
        assert!(body.contains("cannot be undone"));
    }

    #[test]
    fn confirmation_carries_the_safe_mode_note_for_self_protecting_products() {
        let avast = RemovalPlan::for_product(Product::Avast);
        assert!(confirmation_body(&avast).contains("Safe Mode"));

        let mcafee = RemovalPlan::for_product(Product::McAfee);
        assert!(!confirmation_body(&mcafee).contains("Safe Mode"));
    }

    // ── Plan details ─────────────────────────────────────────────────────────

    #[test]
    fn plan_details_covers_every_category_with_counts() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let details = plan_details(&plan);

        for heading in [
            "Folders to delete",
            "Driver files to delete",
            "Services to stop and remove",
            "Scheduled tasks to delete",
            "Registry keys to delete",
        ] {
            assert!(details.contains(heading), "missing section: {heading}");
        }

        assert!(details.contains(&format!("({})", plan.services.len())));
    }

    #[test]
    fn plan_details_lists_real_paths() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let details = plan_details(&plan);
        let first_directory = plan
            .file_paths
            .iter()
            .find(|file| file.is_dir)
            .expect("Avast plan has directories");

        assert!(details.contains(&first_directory.path));
    }

    #[test]
    fn plan_details_truncates_long_categories() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let details = plan_details(&plan);

        assert!(plan.registry_entries.len() > MAX_DETAIL_LINES_PER_CATEGORY);
        assert!(details.contains("…and"));
        assert!(details.contains("more"));
    }

    #[test]
    fn plan_details_does_not_truncate_short_categories() {
        let section = detail_section("Services", &["a", "b"]);
        assert_eq!(section, "Services (2):\n    a\n    b");
    }

    #[test]
    fn a_category_that_exactly_fills_the_cap_is_not_marked_truncated() {
        let entries: Vec<String> = (0..MAX_DETAIL_LINES_PER_CATEGORY)
            .map(|index| format!("entry{index}"))
            .collect();
        let borrowed: Vec<&str> = entries.iter().map(String::as_str).collect();

        let section = detail_section("Keys", &borrowed);
        assert!(
            !section.contains("…and"),
            "a full-but-not-over category must not claim there is more: {section}"
        );
        assert_eq!(section.lines().count(), MAX_DETAIL_LINES_PER_CATEGORY + 1);
    }

    #[test]
    fn one_entry_past_the_cap_reports_exactly_one_more() {
        let entries: Vec<String> = (0..=MAX_DETAIL_LINES_PER_CATEGORY)
            .map(|index| format!("entry{index}"))
            .collect();
        let borrowed: Vec<&str> = entries.iter().map(String::as_str).collect();

        assert!(detail_section("Keys", &borrowed).contains("…and 1 more"));
    }

    #[test]
    fn folders_and_driver_files_are_listed_separately() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let details = plan_details(&plan);
        let (folders, drivers) = details
            .split_once("Driver files to delete")
            .expect("both sections present");

        assert!(
            !folders.contains(".sys"),
            "driver images must not be listed as folders: {folders}"
        );
        let drivers_section = drivers
            .split_once("Services to stop and remove")
            .map_or(drivers, |(section, _)| section);
        for line in drivers_section
            .lines()
            .filter(|line| line.starts_with("    "))
        {
            assert!(
                line.trim().ends_with(".sys"),
                "only driver images belong in this section: {line}"
            );
        }
    }

    #[test]
    fn plan_details_handles_an_empty_category() {
        assert_eq!(detail_section("Services", &[]), "Services (0):");
    }

    #[test]
    fn plan_details_stays_short_enough_for_a_dialog() {
        // A task dialog does not scroll its expanded pane, so the details must
        // fit on screen: 5 sections of at most 8 entries, plus headings.
        for &product in Product::all() {
            let details = plan_details(&RemovalPlan::for_product(product));
            let lines = details.lines().count();
            assert!(lines <= 55, "{product}: details pane is {lines} lines");
        }
    }

    // ── Progress ─────────────────────────────────────────────────────────────

    #[test]
    fn progress_names_the_phase_and_position() {
        let text = progress_body(RemovalPhase::Files, 12, 40);
        assert!(text.contains(RemovalPhase::Files.description()));
        assert!(text.contains("12 of 40"));
    }

    #[test]
    fn progress_heading_names_the_product() {
        let plan = RemovalPlan::for_product(Product::Avg);
        assert!(progress_heading(&plan).contains(Product::Avg.display_name()));
    }

    // ── Report ───────────────────────────────────────────────────────────────

    #[test]
    fn success_report_says_removed_and_offers_no_details() {
        let report = report(10, Vec::new(), Vec::new());
        assert!(report_heading(&report).contains("was removed"));
        assert!(report_body(&report).contains(RESTART_ADVICE));
        assert_eq!(report_details(&report), None);
        assert_eq!(report_footer(&report), None);
    }

    #[test]
    fn partial_report_separates_failures_from_safety_skips() {
        let report = report(
            7,
            vec![r"C:\Windows\System32\drivers\aswSP.sys: left in place".into()],
            vec!["aswSP: Access is denied".into()],
        );

        assert!(report_heading(&report).contains("partly removed"));

        let body = report_body(&report);
        assert!(body.contains("1 failed"));
        assert!(body.contains("keep Windows bootable"));

        let details = report_details(&report).expect("details for a partial run");
        assert!(details.starts_with("Failed:"), "{details}");
        assert!(details.contains("Access is denied"));
        assert!(
            details.contains("\n\nSkipped for safety:"),
            "a blank line must separate the two sections: {details}"
        );
        assert!(details.contains("aswSP.sys"));

        assert!(report_footer(&report).unwrap().contains("Safe Mode"));
    }

    #[test]
    fn a_clean_run_never_mentions_skipped_actions() {
        let body = report_body(&report(10, Vec::new(), Vec::new()));
        assert!(
            !body.contains("skipped"),
            "nothing was skipped, so the report must not say so: {body}"
        );
        assert!(!body.contains("failed"));
    }

    #[test]
    fn a_run_with_only_failures_does_not_claim_anything_was_skipped() {
        let report = report(4, Vec::new(), vec!["a: denied".into()]);
        assert!(report_body(&report).contains("1 failed"));
        assert!(!report_body(&report).contains("skipped"));

        let details = report_details(&report).expect("details for a failed run");
        assert!(details.contains("Failed:"));
        assert!(
            !details.contains("Skipped for safety"),
            "no work was skipped: {details}"
        );
    }

    #[test]
    fn total_failure_is_not_described_as_partial_success() {
        let report = report(0, Vec::new(), vec!["everything: denied".into()]);
        assert!(report_heading(&report).contains("could not be removed"));
    }

    #[test]
    fn report_details_lists_skips_even_with_no_errors() {
        let report = report(9, vec!["driver: left in place".into()], Vec::new());
        let details = report_details(&report).expect("details when work was skipped");
        assert!(
            details.starts_with("Skipped for safety:"),
            "with no failures the pane must not open with blank lines: {details:?}"
        );
        assert!(!details.contains("Failed:"));
    }
}
