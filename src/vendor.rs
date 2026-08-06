//! Running a product's own uninstaller: the route past self-protection.
//!
//! Author: PratikP1
//!
//! Self-protection cannot block a product's *own* uninstaller, or the product
//! would be unremovable.  So the most reliable way past it (without Safe Mode,
//! and without operating the vendor's inaccessible settings UI) is to read the
//! uninstall command the product registered under `…\Uninstall\…` and run it.
//!
//! The policy is accessibility-first and encoded in the tested core here: run a
//! vendor uninstaller **only when it is already silent**.  Guessing a silent
//! switch risks leaving the uninstaller blocking on a dialog a screen-reader
//! user then cannot dismiss: the very trap this feature removes.  Whatever the
//! vendor uninstaller leaves behind is swept by the standard removal that
//! follows, which is Wixen's existing strength.

use crate::executor::ActionOutcome;
use crate::uninstall::UninstallCommand;

// ─── The boundary ────────────────────────────────────────────────────────────

/// Reading a product's registered uninstall command, and running it.
pub trait VendorUninstaller {
    /// The best uninstall string at `uninstall_key`, or `None` if absent.
    ///
    /// The Windows implementation prefers `QuietUninstallString` (which is
    /// silent by definition) and falls back to `UninstallString`.
    fn read_uninstall_string(&self, uninstall_key: &str) -> Option<String>;

    /// Run a parsed, silent uninstall command and report the outcome.
    fn run(&self, command: &UninstallCommand) -> ActionOutcome;
}

// ─── Per-key outcome ─────────────────────────────────────────────────────────

/// What happened when a single uninstall key was probed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorOutcome {
    /// A silent command was run; carries its result.
    Ran(ActionOutcome),
    /// A string was present but not safely silent, so it was not run. Running
    /// it could block on a dialog.
    SkippedNotSilent,
    /// A string was present but could not be parsed.
    Unparsable(String),
    /// No uninstall string at this key.
    Absent,
}

/// Probe each uninstall key and, where the registered command is silent, run
/// it.  Returns the per-key outcomes, paired with the key, for the report.
pub fn run_vendor_uninstallers<'a>(
    uninstall_keys: impl Iterator<Item = &'a str>,
    vendor: &dyn VendorUninstaller,
) -> Vec<(String, VendorOutcome)> {
    uninstall_keys
        .map(|key| (key.to_owned(), probe_and_run(key, vendor)))
        .collect()
}

fn probe_and_run(key: &str, vendor: &dyn VendorUninstaller) -> VendorOutcome {
    let Some(raw) = vendor.read_uninstall_string(key) else {
        return VendorOutcome::Absent;
    };

    match UninstallCommand::parse(&raw) {
        Err(error) => VendorOutcome::Unparsable(error.to_string()),
        Ok(command) if command.is_silent() => VendorOutcome::Ran(vendor.run(&command)),
        // Present but not silent: declining to run is the safe choice.
        Ok(_) => VendorOutcome::SkippedNotSilent,
    }
}

// ─── Reading a registry value (pure parsing + Windows I/O) ────────────────────

/// The value names that hold an uninstall command, best first.  A
/// `QuietUninstallString` is silent by definition, so it is preferred.
///
/// Called only from the Windows module, but kept out here (and un-gated) so
/// the parsing below is tested on Linux; hence the non-Windows allow.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const UNINSTALL_VALUE_NAMES: &[&str] = &["QuietUninstallString", "UninstallString"];

/// Extract the string value `value_name` from the text `reg.exe query` prints.
///
/// `reg query` output looks like:
///
/// ```text
/// HKEY_LOCAL_MACHINE\…\Avast Antivirus
///     QuietUninstallString    REG_SZ    "C:\…\Instup.exe" /silent
/// ```
///
/// Pure and tested so the parsing is verified even though invoking `reg.exe` is
/// not.  Returns `None` when the value is absent.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_reg_query_value(output: &str, value_name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix(value_name)?;
        // The value name must be followed by whitespace, or `UninstallString`
        // would also match a line for `QuietUninstallString`.
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        // ...<type>...<value>; the type is REG_SZ or REG_EXPAND_SZ.
        let (_, after_type) = rest.split_once("REG_")?;
        let value = after_type
            .split_once(char::is_whitespace)
            .map_or("", |(_, value)| value)
            .trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// The real vendor uninstaller.
///
/// Reads the uninstall string with `reg.exe query` (the same shell-out pattern
/// the executor uses for `reg`/`sc`/`schtasks`, avoiding registry FFI) and runs
/// the parsed command.  Off Windows it registers nothing and runs nothing.
pub struct LiveVendorUninstaller;

impl VendorUninstaller for LiveVendorUninstaller {
    fn read_uninstall_string(&self, uninstall_key: &str) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            windows::read_uninstall_string(uninstall_key)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = uninstall_key;
            None
        }
    }

    fn run(&self, command: &UninstallCommand) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::run(command)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = command;
            ActionOutcome::NotFound
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{UNINSTALL_VALUE_NAMES, parse_reg_query_value};
    use crate::executor::ActionOutcome;
    use crate::executor::windows::system_tool_path;
    use crate::uninstall::UninstallCommand;
    use std::path::PathBuf;
    use std::process::Command;

    /// Read the best available uninstall string for `uninstall_key`, preferring
    /// `QuietUninstallString`.
    pub fn read_uninstall_string(uninstall_key: &str) -> Option<String> {
        let reg = system_tool_path("reg.exe").ok()?;
        UNINSTALL_VALUE_NAMES.iter().find_map(|value_name| {
            let output = Command::new(&reg)
                .args(["query", uninstall_key, "/v", value_name])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            parse_reg_query_value(&String::from_utf8_lossy(&output.stdout), value_name)
        })
    }

    /// Run a parsed uninstall command and report the outcome.
    ///
    /// A bare program name (the `msiexec.exe` an MSI string normalizes to) is
    /// resolved to `System32` first: launched elevated through the process
    /// search path, a same-named binary planted earlier in that order could
    /// hijack it. An anchored path runs as the vendor registered it, and a bare
    /// name that cannot be resolved is refused rather than run unqualified.
    pub fn run(command: &UninstallCommand) -> ActionOutcome {
        let program: PathBuf = if command.program_is_bare_name() {
            match system_tool_path(&command.program) {
                Ok(path) => path,
                Err(error) => {
                    return ActionOutcome::Error(format!(
                        "refusing to run {} unqualified: {error}",
                        command.program
                    ));
                }
            }
        } else {
            PathBuf::from(&command.program)
        };

        match Command::new(&program).args(&command.args).status() {
            Ok(status) if status.success() => ActionOutcome::Removed,
            Ok(status) => ActionOutcome::Error(format!(
                "{} exited with {}",
                command.program,
                status
                    .code()
                    .map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
            )),
            Err(error) => {
                ActionOutcome::Error(format!("could not run {}: {error}", command.program))
            }
        }
    }
}

// ─── Stub for tests ──────────────────────────────────────────────────────────

/// A `VendorUninstaller` scripted with a fixed key→string map, recording every
/// command it is asked to run.
#[cfg(any(test, feature = "test-utils"))]
pub struct ScriptedVendorUninstaller {
    strings: Vec<(String, String)>,
    run_outcome: ActionOutcome,
    ran: std::sync::Mutex<Vec<UninstallCommand>>,
}

#[cfg(any(test, feature = "test-utils"))]
impl ScriptedVendorUninstaller {
    /// No product exposes an uninstall string.
    pub fn nothing_registered() -> Self {
        Self {
            strings: Vec::new(),
            run_outcome: ActionOutcome::Removed,
            ran: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// `key` returns `string`; running any command yields `run_outcome`.
    pub fn with_string(key: &str, string: &str, run_outcome: ActionOutcome) -> Self {
        Self {
            strings: vec![(key.to_owned(), string.to_owned())],
            run_outcome,
            ran: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// The commands this stub was actually asked to run.
    pub fn commands_run(&self) -> Vec<UninstallCommand> {
        self.ran.lock().unwrap().clone()
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl VendorUninstaller for ScriptedVendorUninstaller {
    fn read_uninstall_string(&self, uninstall_key: &str) -> Option<String> {
        self.strings
            .iter()
            .find(|(key, _)| key == uninstall_key)
            .map(|(_, string)| string.clone())
    }

    fn run(&self, command: &UninstallCommand) -> ActionOutcome {
        self.ran.lock().unwrap().push(command.clone());
        self.run_outcome.clone()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Antivirus";

    fn outcomes(vendor: &dyn VendorUninstaller, keys: &[&str]) -> Vec<VendorOutcome> {
        run_vendor_uninstallers(keys.iter().copied(), vendor)
            .into_iter()
            .map(|(_, outcome)| outcome)
            .collect()
    }

    // ── a silent string is run ───────────────────────────────────────────────

    #[test]
    fn a_silent_uninstall_string_is_run() {
        let vendor = ScriptedVendorUninstaller::with_string(
            KEY,
            r#""C:\Program Files\AVAST Software\Avast\setup\Instup.exe" /instop:uninstall /silent"#,
            ActionOutcome::Removed,
        );

        assert_eq!(
            outcomes(&vendor, &[KEY]),
            vec![VendorOutcome::Ran(ActionOutcome::Removed)]
        );
        let ran = vendor.commands_run();
        assert_eq!(ran.len(), 1);
        assert!(ran[0].program.ends_with("Instup.exe"));
    }

    #[test]
    fn an_msi_string_is_normalized_and_run() {
        let vendor = ScriptedVendorUninstaller::with_string(
            KEY,
            "MsiExec.exe /X{A7C3D2F1-8E4B-4C9A-B5D6-1F2E3A4B5C6D}",
            ActionOutcome::Removed,
        );
        assert_eq!(
            outcomes(&vendor, &[KEY]),
            vec![VendorOutcome::Ran(ActionOutcome::Removed)]
        );
        assert_eq!(vendor.commands_run()[0].program, "msiexec.exe");
    }

    #[test]
    fn a_vendor_uninstaller_error_is_reported_not_hidden() {
        let vendor = ScriptedVendorUninstaller::with_string(
            KEY,
            r"C:\x\uninst.exe /S",
            ActionOutcome::Error("the uninstaller failed".to_owned()),
        );
        assert_eq!(
            outcomes(&vendor, &[KEY]),
            vec![VendorOutcome::Ran(ActionOutcome::Error(
                "the uninstaller failed".to_owned()
            ))]
        );
    }

    // ── a non-silent string is not run ───────────────────────────────────────

    #[test]
    fn a_non_silent_string_is_skipped_and_never_run() {
        let vendor = ScriptedVendorUninstaller::with_string(
            KEY,
            r"C:\x\uninst.exe /remove",
            ActionOutcome::Removed,
        );
        assert_eq!(
            outcomes(&vendor, &[KEY]),
            vec![VendorOutcome::SkippedNotSilent]
        );
        assert!(
            vendor.commands_run().is_empty(),
            "running a non-silent uninstaller could block on a dialog"
        );
    }

    // ── missing and malformed strings ────────────────────────────────────────

    #[test]
    fn a_key_with_no_string_is_absent() {
        let vendor = ScriptedVendorUninstaller::nothing_registered();
        assert_eq!(outcomes(&vendor, &[KEY]), vec![VendorOutcome::Absent]);
    }

    #[test]
    fn an_unparsable_string_is_reported_not_run() {
        let vendor =
            ScriptedVendorUninstaller::with_string(KEY, r#""unterminated"#, ActionOutcome::Removed);
        assert!(matches!(
            outcomes(&vendor, &[KEY]).as_slice(),
            [VendorOutcome::Unparsable(_)]
        ));
        assert!(vendor.commands_run().is_empty());
    }

    // ── multiple keys ────────────────────────────────────────────────────────

    // ── reg query output parsing ─────────────────────────────────────────────

    const QUERY_OUTPUT: &str = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\…\\Avast Antivirus\r\n    QuietUninstallString    REG_SZ    \"C:\\Program Files\\AVAST Software\\Avast\\setup\\Instup.exe\" /instop:uninstall /silent\r\n\r\n";

    #[test]
    fn a_quiet_uninstall_string_is_read_from_reg_output() {
        let value = parse_reg_query_value(QUERY_OUTPUT, "QuietUninstallString")
            .expect("the value is present");
        assert_eq!(
            value,
            r#""C:\Program Files\AVAST Software\Avast\setup\Instup.exe" /instop:uninstall /silent"#
        );
    }

    #[test]
    fn an_absent_value_yields_none() {
        assert_eq!(parse_reg_query_value(QUERY_OUTPUT, "UninstallString"), None);
        assert_eq!(parse_reg_query_value("", "QuietUninstallString"), None);
    }

    #[test]
    fn an_expand_sz_value_is_read_too() {
        let output = "    UninstallString    REG_EXPAND_SZ    %ProgramFiles%\\X\\uninst.exe /S\n";
        assert_eq!(
            parse_reg_query_value(output, "UninstallString").as_deref(),
            Some(r"%ProgramFiles%\X\uninst.exe /S")
        );
    }

    #[test]
    fn the_value_name_must_be_followed_by_whitespace() {
        // `UninstallString` must not match the `QuietUninstallString` line.
        let output = "    QuietUninstallString    REG_SZ    quiet-value\n";
        assert_eq!(parse_reg_query_value(output, "UninstallString"), None);
    }

    #[test]
    fn a_value_name_with_no_value_yields_none() {
        let output = "    UninstallString    REG_SZ    \n";
        assert_eq!(parse_reg_query_value(output, "UninstallString"), None);
    }

    // ── multiple keys ────────────────────────────────────────────────────────

    #[test]
    fn every_key_is_probed_and_paired_with_its_result() {
        let vendor = ScriptedVendorUninstaller::with_string(
            KEY,
            r"C:\x\uninst.exe /S",
            ActionOutcome::Removed,
        );
        let other = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Other";

        let results = run_vendor_uninstallers([KEY, other].into_iter(), &vendor);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, KEY);
        assert!(matches!(results[0].1, VendorOutcome::Ran(_)));
        assert_eq!(results[1].0, other);
        assert_eq!(results[1].1, VendorOutcome::Absent);
    }
}
