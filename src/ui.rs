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
            text.push_str(&format!("OK — {}\n", only.display_name()));
        }
        [first, second] => {
            text.push_str(&format!(
                "Yes — {}\nNo — {}\n",
                first.display_name(),
                second.display_name()
            ));
        }
        _ => {}
    }

    if has_more {
        text.push_str("Cancel — More products");
    } else {
        text.push_str("Cancel — Quit");
    }

    text
}

#[cfg(any(test, target_os = "windows"))]
fn confirmation_prompt_text(plan: &RemovalPlan) -> String {
    let mut text = format!(
        "Ready to remove {}.\n\nThis will attempt {} action(s) and requires Administrator privileges.\n\nSelect OK to start or Cancel to go back.",
        plan.product.display_name(),
        plan.action_count()
    );

    if let Some(note) = plan.product.pre_removal_note() {
        text.push_str("\n\n");
        text.push_str(note);
    }

    text
}

fn report_body(report: &ExecutionReport) -> String {
    let mut body = format!(
        "Product       : {}\nActions tried : {}\nSucceeded     : {}\n",
        report.product_name, report.actions_attempted, report.actions_succeeded
    );

    if report.fully_succeeded() {
        body.push_str("Status        : SUCCESS — all artifacts removed.");
    } else {
        body.push_str(&format!(
            "Status        : PARTIAL — {} error(s):",
            report.errors.len()
        ));
        for err in &report.errors {
            body.push_str(&format!("\n  • {err}"));
        }
    }

    body
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{APP_TITLE, confirmation_prompt_text, report_body, selection_prompt_text};
    use crate::{executor::ExecutionReport, plan::RemovalPlan, product::Product};
    use std::{
        ffi::{OsStr, c_void},
        io, iter,
        os::windows::ffi::OsStrExt,
        ptr,
    };

    type Hwnd = *mut c_void;

    const MB_OK: u32 = 0x0000_0000;
    const MB_OKCANCEL: u32 = 0x0000_0001;
    const MB_YESNOCANCEL: u32 = 0x0000_0003;
    const MB_ICONQUESTION: u32 = 0x0000_0020;
    const MB_ICONWARNING: u32 = 0x0000_0030;
    const MB_ICONINFORMATION: u32 = 0x0000_0040;
    const MB_SETFOREGROUND: u32 = 0x0001_0000;

    const IDOK: i32 = 1;
    const IDCANCEL: i32 = 2;
    const IDYES: i32 = 6;
    const IDNO: i32 = 7;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
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

    pub fn show_report(report: &ExecutionReport) -> io::Result<()> {
        let title = if report.fully_succeeded() {
            format!("{APP_TITLE} — Removal complete")
        } else {
            format!("{APP_TITLE} — Removal completed with warnings")
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

    fn to_wide(value: &str) -> Vec<u16> {
        OsStr::new(&value.replace('\n', "\r\n"))
            .encode_wide()
            .chain(iter::once(0))
            .collect()
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
    }

    #[test]
    fn selection_prompt_uses_quit_on_last_page() {
        let text = selection_prompt_text(&[Product::Avast, Product::Avg], false);
        assert!(text.contains(Product::Avast.display_name()));
        assert!(text.contains(Product::Avg.display_name()));
        assert!(text.contains("Cancel — Quit"));
    }

    #[test]
    fn confirmation_prompt_includes_action_count() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        let text = confirmation_prompt_text(&plan);
        assert!(text.contains(Product::McAfee.display_name()));
        assert!(text.contains(&plan.action_count().to_string()));
        assert!(text.contains("Administrator"));
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
            errors: vec!["registry: access denied".into(), "service: timeout".into()],
        };

        let text = report_body(&report);
        assert!(text.contains("PARTIAL"));
        assert!(text.contains("registry: access denied"));
        assert!(text.contains("service: timeout"));
    }
}
