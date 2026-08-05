//! User-interface entry points.
//!
//! On Windows the binary uses native Win32 message boxes so the application can
//! be driven without a terminal.  On other platforms the existing CLI remains
//! available for development and automated testing.

#[cfg(not(target_os = "windows"))]
use crate::menu::run_menu;
use crate::{executor::ExecutionReport, plan::RemovalPlan, product::Product};
use std::io;

pub const APP_TITLE: &str = "Wixen Uninstaller";
pub const HELP_FILE_NAME: &str = "WixenUninstallerHelp.html";
#[cfg(any(test, target_os = "windows"))]
const HELP_KEY_GUIDANCE: &str = "F1 opens help.";

/// Select the product to remove.
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

/// Confirm the removal plan before execution starts.
pub fn confirm_plan(plan: &RemovalPlan) -> io::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows::confirm_plan(plan)
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("\nBuilding removal plan for {}…", plan.product);
        println!(
            "Plan contains {} action(s). Starting removal (Administrator privileges required)…\n",
            plan.action_count()
        );
        if let Some(note) = plan.product.pre_removal_note() {
            println!("{note}\n");
        }
        Ok(true)
    }
}

/// Present a blocking error and make no changes.
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
        println!("{}", report_body(report));
        Ok(())
    }
}

#[cfg(any(test, target_os = "windows"))]
fn selection_prompt_text(products: &[Product], has_more: bool) -> String {
    let mut text = String::from("Select a product to remove:\n\n");

    match products {
        [only] => {
            text.push_str(&format!("OK - {}\n", only.display_name()));
        }
        [first, second] => {
            text.push_str(&format!(
                "Yes - {}\nNo - {}\n",
                first.display_name(),
                second.display_name()
            ));
        }
        _ => {}
    }

    if has_more {
        text.push_str("Cancel - More products");
    } else {
        text.push_str("Cancel - Quit");
    }

    text.push_str("\n\nKeyboard: Tab / Shift+Tab moves between buttons. Enter or Space activates the focused button. Esc selects Cancel. ");
    text.push_str(HELP_KEY_GUIDANCE);

    text
}

#[cfg(any(test, target_os = "windows"))]
fn confirmation_prompt_text(plan: &RemovalPlan) -> String {
    let mut text = format!(
        "Ready to remove {}.\n\nThis will attempt {} action(s) and requires Administrator privileges.\n\nKeyboard: Tab / Shift+Tab moves between OK and Cancel. Enter or Space activates the focused button. Esc goes back. {}",
        plan.product.display_name(),
        plan.action_count(),
        HELP_KEY_GUIDANCE
    );

    if let Some(note) = plan.product.pre_removal_note() {
        text.push_str("\n\n");
        text.push_str(note);
    }

    text
}

/// Removing kernel drivers and services only takes full effect after a restart.
const RESTART_ADVICE: &str = "Restart Windows to finish the cleanup.";

fn report_body(report: &ExecutionReport) -> String {
    let mut body = format!(
        "Product       : {}\nActions tried : {}\nSucceeded     : {}\nSkipped       : {}\n",
        report.product_name,
        report.actions_attempted,
        report.actions_succeeded,
        report.actions_skipped()
    );

    if report.fully_succeeded() {
        body.push_str("Status        : SUCCESS - all artifacts removed.");
    } else {
        body.push_str(&format!(
            "Status        : PARTIAL - {} error(s), {} skipped for safety.",
            report.errors.len(),
            report.actions_skipped()
        ));
        append_bullets(&mut body, "Errors", &report.errors);
        append_bullets(&mut body, "Skipped for safety", &report.warnings);
    }

    body.push_str("\n\n");
    body.push_str(RESTART_ADVICE);
    body
}

fn append_bullets(body: &mut String, heading: &str, entries: &[String]) {
    if entries.is_empty() {
        return;
    }

    body.push_str(&format!("\n\n{heading}:"));
    for entry in entries {
        body.push_str(&format!("\n  • {entry}"));
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{
        APP_TITLE, HELP_FILE_NAME, confirmation_prompt_text, report_body, selection_prompt_text,
    };
    use crate::{executor::ExecutionReport, plan::RemovalPlan, product::Product};
    use std::{
        ffi::{OsStr, c_void},
        io, iter,
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
        ptr,
    };

    type Hwnd = *mut c_void;
    type Hinstance = *mut c_void;

    const MB_OK: u32 = 0x0000_0000;
    const MB_OKCANCEL: u32 = 0x0000_0001;
    const MB_YESNOCANCEL: u32 = 0x0000_0003;
    const MB_HELP: u32 = 0x0000_4000;
    const MB_ICONERROR: u32 = 0x0000_0010;
    const MB_ICONQUESTION: u32 = 0x0000_0020;
    const MB_ICONWARNING: u32 = 0x0000_0030;
    const MB_ICONINFORMATION: u32 = 0x0000_0040;
    const MB_SETFOREGROUND: u32 = 0x0001_0000;
    const SW_SHOWNORMAL: i32 = 1;
    const SHELL_OPEN_SUCCESS_THRESHOLD: isize = 32;

    const IDOK: i32 = 1;
    const IDCANCEL: i32 = 2;
    const IDYES: i32 = 6;
    const IDNO: i32 = 7;

    #[allow(non_snake_case)]
    #[repr(C)]
    struct MsgBoxParamsW {
        cbSize: u32,
        hwndOwner: Hwnd,
        hInstance: Hinstance,
        lpszText: *const u16,
        lpszCaption: *const u16,
        dwStyle: u32,
        lpszIcon: *const u16,
        dwContextHelpId: usize,
        lpfnMsgBoxCallback: MsgBoxCallback,
        dwLanguageId: u32,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct HelpInfo {
        cbSize: u32,
        iContextType: i32,
        iCtrlId: i32,
        hItemHandle: *mut c_void,
        dwContextId: usize,
    }

    type MsgBoxCallback = Option<unsafe extern "system" fn(*const HelpInfo)>;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
        fn MessageBoxIndirectW(params: *const MsgBoxParamsW) -> i32;
    }

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

    pub fn select_product() -> io::Result<Option<Product>> {
        let products = Product::all();
        if products.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Win32 selection UI requires at least one supported product",
            ));
        }

        let mut index = 0usize;
        while index < products.len() {
            let end = (index + 2).min(products.len());
            let page = &products[index..end];
            let has_more = end < products.len();
            let buttons = if page.len() == 1 {
                MB_OKCANCEL
            } else {
                MB_YESNOCANCEL
            };

            let selection = show_message(
                &selection_prompt_text(page, has_more),
                APP_TITLE,
                buttons | MB_ICONQUESTION | MB_SETFOREGROUND,
            )?;

            match (page, selection, has_more) {
                ([only], IDOK, _) => return Ok(Some(*only)),
                ([first, _], IDYES, _) => return Ok(Some(*first)),
                ([_, second], IDNO, _) => return Ok(Some(*second)),
                (_, IDCANCEL, true) => index = end,
                (_, IDCANCEL, false) => return Ok(None),
                _ => return Ok(None),
            }
        }

        Ok(None)
    }

    pub fn confirm_plan(plan: &RemovalPlan) -> io::Result<bool> {
        let selection = show_message(
            &confirmation_prompt_text(plan),
            APP_TITLE,
            MB_OKCANCEL | MB_ICONWARNING | MB_SETFOREGROUND,
        )?;
        Ok(selection == IDOK)
    }

    /// A plain message box without the Help button: shown before the help file
    /// has been proven reachable, so offering F1 would be a dead end.
    pub fn show_error(message: &str) -> io::Result<()> {
        show_plain_message(message, APP_TITLE, MB_OK | MB_ICONERROR | MB_SETFOREGROUND).map(|_| ())
    }

    pub fn show_report(report: &ExecutionReport) -> io::Result<()> {
        let title = if report.fully_succeeded() {
            format!("{APP_TITLE} - Removal complete")
        } else {
            format!("{APP_TITLE} - Removal completed with warnings")
        };
        let icon = if report.fully_succeeded() {
            MB_ICONINFORMATION
        } else {
            MB_ICONWARNING
        };

        show_message(
            &report_body(report),
            &title,
            MB_OK | icon | MB_SETFOREGROUND,
        )
        .map(|_| ())
    }

    fn show_message(text: &str, title: &str, kind: u32) -> io::Result<i32> {
        let text_wide = to_wide(text);
        let title_wide = to_wide(title);
        let params = MsgBoxParamsW {
            cbSize: std::mem::size_of::<MsgBoxParamsW>() as u32,
            hwndOwner: ptr::null_mut(),
            hInstance: ptr::null_mut(),
            lpszText: text_wide.as_ptr(),
            lpszCaption: title_wide.as_ptr(),
            dwStyle: kind | MB_HELP,
            lpszIcon: ptr::null(),
            dwContextHelpId: 1,
            lpfnMsgBoxCallback: Some(open_help_callback),
            dwLanguageId: 0,
        };
        let result = unsafe { MessageBoxIndirectW(&params) };

        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    unsafe extern "system" fn open_help_callback(_help_info: *const HelpInfo) {
        if let Err(err) = open_help_documentation() {
            let message = format!(
                "Unable to open the help file.\n\nExpected to find {} next to the executable.\n\n{}",
                HELP_FILE_NAME, err
            );
            let _ = show_plain_message(
                &message,
                APP_TITLE,
                MB_OK | MB_ICONWARNING | MB_SETFOREGROUND,
            );
        }
    }

    fn open_help_documentation() -> io::Result<()> {
        let help_path = find_help_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} was not found", HELP_FILE_NAME),
            )
        })?;

        shell_open(&help_path)
    }

    /// Locate the bundled help file.
    ///
    /// Only the directory holding the executable is searched.  Wixen runs
    /// elevated and hands this path straight to `ShellExecuteW`, so searching
    /// ancestor directories would let anyone who can write to a parent decide
    /// what an elevated browser opens — and the root of the system drive is
    /// writable by authenticated users on a default Windows install.
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
    /// `cargo run` can open help.  The location is baked in at compile time
    /// rather than discovered at run time, and release builds omit it entirely.
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

    fn shell_open(path: &Path) -> io::Result<()> {
        let operation_wide = to_wide("open");
        let file_wide = to_wide_os(path.as_os_str());
        let directory_wide = path.parent().map(|parent| to_wide_os(parent.as_os_str()));
        let directory = directory_wide
            .as_ref()
            .map_or(ptr::null(), |value| value.as_ptr());
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation_wide.as_ptr(),
                file_wide.as_ptr(),
                ptr::null(),
                directory,
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

    fn show_plain_message(text: &str, title: &str, kind: u32) -> io::Result<i32> {
        let text_wide = to_wide(text);
        let title_wide = to_wide(title);
        let result = unsafe {
            MessageBoxW(
                ptr::null_mut(),
                text_wide.as_ptr(),
                title_wide.as_ptr(),
                kind,
            )
        };

        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    /// Message boxes want CRLF line endings, and our strings use plain LF.
    /// Collapsing first keeps text that already contains CRLF — such as an
    /// error message handed back by Windows — from gaining a stray carriage
    /// return.
    fn to_wide(value: &str) -> Vec<u16> {
        let crlf = value.replace("\r\n", "\n").replace('\n', "\r\n");
        OsStr::new(&crlf)
            .encode_wide()
            .chain(iter::once(0))
            .collect()
    }

    fn to_wide_os(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(iter::once(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{executor::ExecutionReport, plan::RemovalPlan, product::Product};

    #[test]
    fn selection_prompt_mentions_both_products() {
        let text = selection_prompt_text(&[Product::McAfee, Product::Norton], true);
        assert!(text.contains(Product::McAfee.display_name()));
        assert!(text.contains(Product::Norton.display_name()));
        assert!(text.contains("More products"));
        assert!(text.contains("Tab / Shift+Tab"));
        assert!(text.contains("Esc"));
    }

    #[test]
    fn selection_prompt_uses_quit_on_last_page() {
        let text = selection_prompt_text(&[Product::Avast, Product::Avg], false);
        assert!(text.contains(Product::Avast.display_name()));
        assert!(text.contains(Product::Avg.display_name()));
        assert!(text.contains("Cancel - Quit"));
    }

    #[test]
    fn selection_prompt_single_product_includes_keyboard_guidance() {
        let text = selection_prompt_text(&[Product::McAfee], false);
        assert!(text.contains("OK"));
        assert!(text.contains("Enter or Space"));
        assert!(text.contains("Esc"));
        assert!(text.contains("F1"));
    }

    #[test]
    fn confirmation_prompt_includes_action_count() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        let text = confirmation_prompt_text(&plan);
        assert!(text.contains(Product::McAfee.display_name()));
        assert!(text.contains(&plan.action_count().to_string()));
        assert!(text.contains("Administrator"));
        assert!(text.contains("Enter or Space"));
        assert!(text.contains("Esc"));
        assert!(text.contains("F1"));
    }

    #[test]
    fn confirmation_prompt_includes_safe_mode_note_when_needed() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let text = confirmation_prompt_text(&plan);
        assert!(text.contains("Safe Mode"));
    }

    #[test]
    fn success_report_body_has_success_status() {
        let report = ExecutionReport {
            product_name: Product::Norton.display_name().to_owned(),
            actions_attempted: 5,
            actions_succeeded: 5,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        let text = report_body(&report);
        assert!(text.contains("Status"));
        assert!(text.contains("SUCCESS"));
        assert!(!text.contains("PARTIAL"));
    }

    #[test]
    fn partial_report_body_lists_errors() {
        let report = ExecutionReport {
            product_name: Product::McAfee.display_name().to_owned(),
            actions_attempted: 4,
            actions_succeeded: 2,
            warnings: Vec::new(),
            errors: vec!["registry: access denied".into(), "service: timeout".into()],
        };

        let text = report_body(&report);
        assert!(text.contains("PARTIAL"));
        assert!(text.contains("registry: access denied"));
        assert!(text.contains("service: timeout"));
    }

    #[test]
    fn skipped_actions_are_reported_separately_from_errors() {
        let report = ExecutionReport {
            product_name: Product::Avast.display_name().to_owned(),
            actions_attempted: 6,
            actions_succeeded: 4,
            warnings: vec![r"C:\Windows\System32\drivers\aswSP.sys: left in place".into()],
            errors: vec!["aswSP: Access is denied".into()],
        };

        let text = report_body(&report);
        assert!(text.contains("Skipped for safety"));
        assert!(text.contains("aswSP.sys"));
        assert!(text.contains("Errors"));
        assert!(text.contains("Access is denied"));
        assert!(text.contains("Skipped       : 1"));
    }

    #[test]
    fn every_report_asks_the_user_to_restart() {
        let report = ExecutionReport {
            product_name: Product::Avg.display_name().to_owned(),
            actions_attempted: 1,
            actions_succeeded: 1,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        assert!(report_body(&report).contains(RESTART_ADVICE));
    }
}
