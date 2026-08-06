//! The Windows screens, built from native task dialogs.
//!
//! Author: PratikP1
//!
//! Each screen is assembled from the wording in [`crate::ui`] and handed to
//! [`super::task_dialog`].  Nothing here decides what to *say* — that lives in
//! the parent module, where it can be tested on any platform.

use super::task_dialog::{
    Dialog, IDCANCEL, IDOK, Icon, ProgressState, TDCBF_CANCEL, TDCBF_CLOSE, set_help_handler,
    show_progress,
};
use crate::{
    executor::{ExecutionReport, Executor, RemovalPhase, execute_full_with_progress},
    forceful::ForcefulExecutor,
    plan::RemovalPlan,
    product::Product,
    resume::ResumeState,
    ui::{
        APP_TITLE, HELP_FILE_NAME, HELP_FOOTER, RESTART_SCHEDULED_HEADING, RUNNING_UNINSTALLER,
        confirmation_body, confirmation_heading, plan_details, progress_heading, report_body,
        report_details, report_footer, report_heading, restart_scheduled_body, selection_body,
        selection_heading, system_wait_body,
    },
    vendor::VendorUninstaller,
};
use std::{
    ffi::{OsStr, c_void},
    io, iter,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};

/// Command-link ids start above the standard `ID*` values so a product choice
/// can never be confused with Cancel.
const PRODUCT_BUTTON_BASE: i32 = 100;

// ─── Screens ─────────────────────────────────────────────────────────────────

/// Product selection: one large labelled button per product.
///
/// Every product is its own command link, so a screen reader announces the
/// product name and what else its cleanup sweeps up.  Arrow keys and Tab move
/// between them, and each one is reachable directly.
pub fn select_product() -> io::Result<Option<Product>> {
    register_help();

    let products = Product::all();
    let mut dialog = Dialog::new(APP_TITLE, selection_heading(), selection_body())
        .common_buttons(TDCBF_CANCEL)
        .footer(HELP_FOOTER);

    for (index, product) in products.iter().enumerate() {
        dialog = dialog.command_link(
            PRODUCT_BUTTON_BASE + index as i32,
            product.display_name(),
            product.companion_note(),
        );
    }

    let pressed = dialog.show()?;
    let Ok(choice) = usize::try_from(pressed - PRODUCT_BUTTON_BASE) else {
        return Ok(None);
    };

    Ok(products.get(choice).copied())
}

/// Confirmation: what will happen, with the full list one keystroke away.
pub fn confirm_plan(plan: &RemovalPlan) -> io::Result<bool> {
    register_help();

    let pressed = Dialog::new(
        APP_TITLE,
        &confirmation_heading(plan),
        &confirmation_body(plan),
    )
    .icon(Icon::Warning)
    .button(IDOK, "&Remove it")
    .common_buttons(TDCBF_CANCEL)
    // Focus starts on Cancel: pressing Enter by reflex must not begin an
    // irreversible removal.
    .default_button(IDCANCEL)
    .details(
        "Show what will be removed",
        "Hide what will be removed",
        &plan_details(plan),
    )
    .footer(HELP_FOOTER)
    .show()?;

    Ok(pressed == IDOK)
}

/// Tells the progress dialog it may close, however the removal ends.
struct FinishOnDrop<'a>(&'a ProgressState);

impl Drop for FinishOnDrop<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// Run the full escalating removal on a worker thread while a progress dialog
/// reports on it.
///
/// Without this the window simply stops responding for the length of the
/// removal, which for a screen-reader user is indistinguishable from a crash.
/// The dialog opens on the uninstaller step — the one opaque phase — and the
/// bar begins to move once the guarded sweep starts.
pub fn run_full_removal(
    plan: &RemovalPlan,
    vendor: &(dyn VendorUninstaller + Sync),
    executor: &(dyn Executor + Sync),
    forceful: &(dyn ForcefulExecutor + Sync),
) -> io::Result<(ExecutionReport, Option<ResumeState>)> {
    register_help();

    let total = plan.action_count();
    let state = ProgressState::new();
    let phase_text: Vec<String> = RemovalPhase::all()
        .iter()
        .map(|phase| phase.description().to_owned())
        .collect();

    let dialog = Dialog::new(APP_TITLE, &progress_heading(plan), RUNNING_UNINSTALLER)
        .common_buttons(TDCBF_CANCEL)
        .allow_cancel(false)
        .footer("Interrupting a removal can leave the product half-installed, so this step cannot be cancelled.");

    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            // Released on the way out however the removal ends. Without this a
            // panic would leave the dialog waiting on a worker that will never
            // report, with its Cancel button disabled and no way out.
            let _release_dialog = FinishOnDrop(&state);
            execute_full_with_progress(plan, vendor, executor, forceful, &mut |phase, completed| {
                state.advance(phase.index(), completed);
            })
        });

        // A failure here means the dialog could not be shown; the removal still
        // runs to completion on the worker, and the report below is what the
        // user needs.  The same failure recurs on the report screen, which does
        // surface it.
        let _ = show_progress(&dialog, &state, total, &phase_text);

        worker
            .join()
            .map_err(|_| io::Error::other("the removal thread panicked"))
    })
}

/// Run the removal as SYSTEM behind a "working" dialog.
///
/// The SYSTEM run is headless — session 0 has no desktop — so no per-item
/// progress comes back.  The bar creeps one step per poll toward, but never
/// reaching, a fixed total: that reads as "still working" without ever claiming
/// to be done, and the body text explains the wait.  Returns `None` when the
/// relaunch could not be completed, so the caller runs the removal in-process.
pub fn run_removal_via_system(
    plan: &RemovalPlan,
    product: Product,
) -> io::Result<Option<(ExecutionReport, bool)>> {
    register_help();

    // At one step per 500ms poll, this saturates after ~5 minutes and then
    // holds near-full until the run reports back.
    const CREEP_TOTAL: usize = 600;
    let state = ProgressState::new();
    let body = system_wait_body(product);
    let phase_text = [body.clone()];

    let dialog = Dialog::new(APP_TITLE, &progress_heading(plan), &body)
        .common_buttons(TDCBF_CANCEL)
        .allow_cancel(false)
        .footer(
            "Wixen is finishing the removal at the highest privilege. Interrupting it \
             can leave the product half-installed, so this step cannot be cancelled.",
        );

    let outcome = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            // Released however the run ends, so a failure or panic still lets the
            // dialog close instead of stranding it with Cancel disabled.
            let _release_dialog = FinishOnDrop(&state);
            let mut ticks = 0usize;
            crate::system_exec::run_execution_as_system(product, || {
                ticks = ticks.saturating_add(1);
                state.advance(0, ticks.min(CREEP_TOTAL - 1));
            })
        });

        let _ = show_progress(&dialog, &state, CREEP_TOTAL, &phase_text);

        worker
            .join()
            .map_err(|_| io::Error::other("the SYSTEM-wait thread panicked"))
    })?;

    // Err means the relaunch could not be completed (task refused, timed out, or
    // wrote no readable result); the caller falls back to an in-process removal.
    Ok(outcome.ok())
}

/// Shown when files were queued for boot-time deletion: the removal will finish
/// on its own after a normal restart, so the user is never left wondering.
pub fn show_restart_scheduled() -> io::Result<()> {
    register_help();

    Dialog::new(
        APP_TITLE,
        RESTART_SCHEDULED_HEADING,
        restart_scheduled_body(),
    )
    .icon(Icon::Information)
    .common_buttons(TDCBF_CLOSE)
    .footer(HELP_FOOTER)
    .show()
    .map(|_| ())
}

/// The final report, with failures and safety skips behind "Show details".
pub fn show_report(report: &ExecutionReport) -> io::Result<()> {
    register_help();

    let mut dialog = Dialog::new(APP_TITLE, &report_heading(report), &report_body(report))
        .icon(if report.fully_succeeded() {
            Icon::Information
        } else {
            Icon::Warning
        })
        .common_buttons(TDCBF_CLOSE);

    if let Some(details) = report_details(report) {
        dialog = dialog.details("Show details", "Hide details", &details);
    }

    dialog = match report_footer(report) {
        Some(footer) => dialog.footer(footer),
        None => dialog.footer(HELP_FOOTER),
    };

    dialog.show().map(|_| ())
}

/// A blocking error shown before anything has been changed.
pub fn show_error(message: &str) -> io::Result<()> {
    let (heading, body) = message.split_once("\n\n").unwrap_or((message, ""));

    Dialog::new(APP_TITLE, heading, body)
        .icon(Icon::Shield)
        .common_buttons(TDCBF_CLOSE)
        .show()
        .map(|_| ())
}

// ─── Help ────────────────────────────────────────────────────────────────────

fn register_help() {
    set_help_handler(open_help);
}

fn open_help() {
    if let Err(error) = open_help_documentation() {
        let _ = Dialog::new(
            APP_TITLE,
            "The help file could not be opened.",
            &format!(
                "{HELP_FILE_NAME} should sit next to the Wixen Uninstaller program.\n\n{error}"
            ),
        )
        .icon(Icon::Warning)
        .common_buttons(TDCBF_CLOSE)
        .show();
    }
}

fn open_help_documentation() -> io::Result<()> {
    let help_path = find_help_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{HELP_FILE_NAME} was not found"),
        )
    })?;

    shell_open(&help_path)
}

/// Locate the bundled help file.
///
/// Only the directory holding the executable is searched.  Wixen runs elevated
/// and hands this path straight to `ShellExecuteW`, so searching ancestor
/// directories would let anyone who can write to a parent decide what an
/// elevated browser opens — and the root of the system drive is writable by
/// authenticated users on a default Windows install.
fn find_help_path() -> Option<PathBuf> {
    let beside_executable = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::parent)
        .map(|directory| directory.join(HELP_FILE_NAME))
        .filter(|path| path.is_file());

    beside_executable.or_else(development_help_path)
}

/// In debug builds only, fall back to the checked-out `docs/` folder so
/// `cargo run` can open help.  The location is baked in at compile time rather
/// than discovered at run time, and release builds omit it entirely.
fn development_help_path() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs")
            .join(HELP_FILE_NAME);
        path.is_file().then_some(path)
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

type Hwnd = *mut c_void;
type Hinstance = *mut c_void;

const SW_SHOWNORMAL: i32 = 1;
/// `ShellExecuteW` returns a value greater than 32 on success.
const SHELL_OPEN_SUCCESS_THRESHOLD: isize = 32;

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: Hwnd,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> Hinstance;
}

fn shell_open(path: &Path) -> io::Result<()> {
    let operation = to_wide_os(OsStr::new("open"));
    let file = to_wide_os(path.as_os_str());
    let directory = path.parent().map(|parent| to_wide_os(parent.as_os_str()));
    let directory_ptr = directory
        .as_ref()
        .map_or(ptr::null(), |value| value.as_ptr());

    // SAFETY: every pointer refers to a NUL-terminated buffer that outlives the
    // call, and a null owner window is valid.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            directory_ptr,
            SW_SHOWNORMAL,
        ) as isize
    };

    if result <= SHELL_OPEN_SUCCESS_THRESHOLD {
        Err(io::Error::other(format!(
            "Windows could not open {}",
            path.display()
        )))
    } else {
        Ok(())
    }
}

fn to_wide_os(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(iter::once(0)).collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_link_ids_map_back_to_products() {
        for (index, &product) in Product::all().iter().enumerate() {
            let id = PRODUCT_BUTTON_BASE + index as i32;
            let choice = usize::try_from(id - PRODUCT_BUTTON_BASE).unwrap();
            assert_eq!(Product::all().get(choice).copied(), Some(product));
        }
    }

    #[test]
    fn cancel_does_not_decode_as_a_product() {
        // IDCANCEL is below the command-link base, so the subtraction is
        // negative and `usize::try_from` rejects it.
        assert!(usize::try_from(IDCANCEL - PRODUCT_BUTTON_BASE).is_err());
        assert!(usize::try_from(IDOK - PRODUCT_BUTTON_BASE).is_err());
    }

    #[test]
    fn an_out_of_range_button_id_selects_nothing() {
        let beyond = PRODUCT_BUTTON_BASE + Product::all().len() as i32;
        let choice = usize::try_from(beyond - PRODUCT_BUTTON_BASE).unwrap();
        assert_eq!(Product::all().get(choice), None);
    }

    #[test]
    fn error_text_splits_into_a_heading_and_body() {
        let message = "Wixen needs Administrator.\n\nRight-click and run as administrator.";
        let (heading, body) = message.split_once("\n\n").unwrap();
        assert_eq!(heading, "Wixen needs Administrator.");
        assert!(body.contains("Right-click"));
    }

    #[test]
    fn a_single_line_error_still_shows_as_a_heading() {
        let (heading, body) = "Something failed."
            .split_once("\n\n")
            .unwrap_or(("Something failed.", ""));
        assert_eq!(heading, "Something failed.");
        assert!(body.is_empty());
    }
}
