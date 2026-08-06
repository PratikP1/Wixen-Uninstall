//! The forceful-removal boundary and the per-file escalation loop.
//!
//! Author: PratikP1
//!
//! [`ForcefulExecutor`] is the seam between the tested escalation *decisions*
//! (`escalation.rs`) and the Windows calls that carry them out: taking
//! ownership of a denied file, or queuing a locked one for boot-time deletion.
//! Keeping it a trait means the loop that drives the ladder,
//! [`resolve_file`], is exercised on Linux against a scripted stub, while the
//! real `MoveFileEx` / take-ownership code lives behind
//! `#[cfg(target_os = "windows")]`.
//!
//! The boot-safety guard is not re-implemented here: every decision routes
//! through [`next_step`], which refuses to force a guarded driver.

use crate::escalation::{NextStep, Remedy, next_step};
use crate::executor::ActionOutcome;
use crate::plan::FilePath;

// ─── The boundary ────────────────────────────────────────────────────────────

/// Forceful operations, used only when ordinary deletion is refused.
///
/// Separate from [`crate::executor::Executor`] so the standard sweep and its
/// many test doubles are untouched: escalation is strictly opt-in.
pub trait ForcefulExecutor {
    /// Take ownership, grant Administrators full control, and delete now.
    fn take_ownership_and_delete(&self, file: &FilePath) -> ActionOutcome;

    /// Queue `file` for deletion at the next boot.  A success means *queued*,
    /// not deleted: the file goes when Windows next starts.
    fn schedule_delete_at_reboot(&self, file: &FilePath) -> ActionOutcome;
}

// ─── Per-file resolution ─────────────────────────────────────────────────────

/// How a single file ended up after the ladder ran to a conclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileResolution {
    /// Gone now: removed outright, or forced after taking ownership.
    Removed,
    /// Deliberately left: a guarded driver, or an already-benign skip.  Carries
    /// the reason for the report.
    Skipped(String),
    /// Queued for deletion at the next boot.  Carries the path, which the
    /// caller records in the resume state.
    Deferred(String),
    /// Could not be removed and no remedy would help.  Carries the reason.
    Failed(String),
}

/// Drive the escalation ladder for one file to a conclusion.
///
/// `initial` is the outcome of the ordinary deletion already attempted by the
/// standard sweep.  Every branch is chosen by [`next_step`], so the driver
/// guard holds without being restated here.  The ladder is finite
/// (`None → TakeOwnership → DelayUntilReboot`), so this cannot loop.
pub fn resolve_file(
    file: &FilePath,
    removed_services: &[&str],
    initial: ActionOutcome,
    forceful: &dyn ForcefulExecutor,
) -> FileResolution {
    let mut outcome = initial;
    let mut last_remedy = None;

    loop {
        match next_step(file, removed_services, last_remedy, &outcome) {
            NextStep::Done => return FileResolution::Removed,
            NextStep::SkipForSafety(reason) => return FileResolution::Skipped(reason),
            NextStep::GiveUp(reason) => return FileResolution::Failed(reason),
            NextStep::Escalate(Remedy::TakeOwnership) => {
                outcome = forceful.take_ownership_and_delete(file);
                last_remedy = Some(Remedy::TakeOwnership);
            }
            NextStep::Escalate(Remedy::DelayUntilReboot) => {
                return resolve_delayed(file, forceful);
            }
        }
    }
}

/// The terminal rung: queue for boot-time deletion, or fail if even that is
/// refused.  Queuing does not delete now, so success is [`Deferred`], never
/// [`Removed`].
///
/// [`Deferred`]: FileResolution::Deferred
/// [`Removed`]: FileResolution::Removed
fn resolve_delayed(file: &FilePath, forceful: &dyn ForcefulExecutor) -> FileResolution {
    match forceful.schedule_delete_at_reboot(file) {
        ActionOutcome::Error(message) => FileResolution::Failed(message),
        // Removed / NotFound both mean the queue accepted the path.
        _ => FileResolution::Deferred(file.path.clone()),
    }
}

// ─── Command construction (pure, tested everywhere) ──────────────────────────
//
// These build the `takeown`/`icacls` argument lists.  They are called from the
// Windows-only module, but kept out of it (and un-gated) so the recursion
// logic is exercised by the test suite and cargo-mutants on Linux.  Off
// Windows nothing but the tests calls them, hence the `cfg_attr` allow.

/// The well-known SID of the local Administrators group.  A SID, not the name
/// "Administrators", because the name is localized and would not match on a
/// non-English Windows.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const ADMINISTRATORS_SID: &str = "*S-1-5-32-544";

/// Arguments for `takeown.exe` to seize `file` for the Administrators group.
///
/// A directory recurses; a single file does not.  The difference decides
/// whether a whole product tree or just one image is taken, so it is pure and
/// tested rather than buried in the Windows call.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn takeown_args(file: &FilePath) -> Vec<String> {
    // `/A` assigns ownership to the Administrators group rather than the running
    // user, so the later `icacls` grant matches.
    let mut args = vec!["/F".to_owned(), file.path.clone(), "/A".to_owned()];
    if file.is_dir {
        // Recurse, answering the "process directory?" prompt with Yes.
        args.extend(["/R".to_owned(), "/D".to_owned(), "Y".to_owned()]);
    }
    args
}

/// Arguments for `icacls.exe` to grant the Administrators group full control.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn icacls_args(file: &FilePath) -> Vec<String> {
    // `/C` continues past individual errors; `/T` recurses for a directory.
    let mut args = vec![
        file.path.clone(),
        "/grant".to_owned(),
        format!("{ADMINISTRATORS_SID}:(F)"),
        "/C".to_owned(),
    ];
    if file.is_dir {
        args.push("/T".to_owned());
    }
    args
}

// ─── Live (Windows) implementation ───────────────────────────────────────────

/// The real forceful executor.
///
/// Compiles on every platform; off Windows each method is a no-op returning
/// `NotFound`, so the escalation ladder and its tests build anywhere.
pub struct LiveForcefulExecutor;

impl ForcefulExecutor for LiveForcefulExecutor {
    fn take_ownership_and_delete(&self, file: &FilePath) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::take_ownership_and_delete(file)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = file;
            ActionOutcome::NotFound
        }
    }

    fn schedule_delete_at_reboot(&self, file: &FilePath) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::schedule_delete_at_reboot(file)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = file;
            ActionOutcome::NotFound
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{ActionOutcome, icacls_args, takeown_args};
    use crate::executor::windows::{delete_file_path, system_tool_path};
    use crate::plan::FilePath;
    use std::{ffi::OsStr, iter, os::windows::ffi::OsStrExt, process::Command};

    /// `MoveFileEx`: delete during early boot instead of now.
    const MOVEFILE_DELAY_UNTIL_REBOOT: u32 = 0x0000_0004;
    /// A path already gone needs no queuing.
    const ERROR_FILE_NOT_FOUND: i32 = 2;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    /// Take ownership, grant Administrators full control, then retry the delete.
    ///
    /// Ownership and the grant are best-effort: `takeown`/`icacls`, the same
    /// built-in tools an administrator would use, invoked by absolute path like
    /// the rest of the executor.  The retry deletion is authoritative: whatever
    /// it reports is the outcome, so a failure to reset the ACL simply lets the
    /// ladder fall through to a delayed delete.
    pub fn take_ownership_and_delete(file: &FilePath) -> ActionOutcome {
        let _ = run_tool("takeown.exe", &takeown_args(file));
        let _ = run_tool("icacls.exe", &icacls_args(file));
        delete_file_path(file)
    }

    fn run_tool(tool: &str, args: &[String]) -> Result<(), String> {
        let path = system_tool_path(tool).map_err(|error| error.to_string())?;
        Command::new(path)
            .args(args)
            .output()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Queue `file` for deletion at the next boot via `MoveFileEx`.
    ///
    /// The session manager performs the deletion during early boot, before the
    /// process that holds the file open has started.  No Safe Mode required.
    pub fn schedule_delete_at_reboot(file: &FilePath) -> ActionOutcome {
        let wide = to_wide(&file.path);

        // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that outlives the
        // call; a null destination with MOVEFILE_DELAY_UNTIL_REBOOT is the
        // documented way to request deletion rather than a move.
        let queued =
            unsafe { MoveFileExW(wide.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT) };

        if queued != 0 {
            // Queued, not yet deleted; the caller records it as deferred.
            return ActionOutcome::Removed;
        }

        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND) {
            ActionOutcome::NotFound
        } else {
            ActionOutcome::Error(format!(
                "could not queue {} for deletion at the next restart: {error}",
                file.path
            ))
        }
    }

    fn to_wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(iter::once(0))
            .collect()
    }
}

// ─── Stub for tests ──────────────────────────────────────────────────────────

/// A `ForcefulExecutor` that returns scripted outcomes, so the ladder can be
/// driven without a Windows machine.
#[cfg(any(test, feature = "test-utils"))]
pub struct ScriptedForcefulExecutor {
    pub ownership_outcome: ActionOutcome,
    pub reboot_outcome: ActionOutcome,
}

#[cfg(any(test, feature = "test-utils"))]
impl ScriptedForcefulExecutor {
    /// Ownership succeeds; nothing needs the reboot queue.
    pub fn ownership_succeeds() -> Self {
        Self {
            ownership_outcome: ActionOutcome::Removed,
            reboot_outcome: ActionOutcome::Removed,
        }
    }

    /// Ownership fails; the reboot queue accepts the path.
    pub fn only_reboot_works() -> Self {
        Self {
            ownership_outcome: ActionOutcome::Error("Access is denied.".to_owned()),
            reboot_outcome: ActionOutcome::Removed,
        }
    }

    /// Nothing works, not even the reboot queue.
    pub fn nothing_works() -> Self {
        Self {
            ownership_outcome: ActionOutcome::Error("Access is denied.".to_owned()),
            reboot_outcome: ActionOutcome::Error("could not queue".to_owned()),
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl ForcefulExecutor for ScriptedForcefulExecutor {
    fn take_ownership_and_delete(&self, _: &FilePath) -> ActionOutcome {
        self.ownership_outcome.clone()
    }

    fn schedule_delete_at_reboot(&self, _: &FilePath) -> ActionOutcome {
        self.reboot_outcome.clone()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn plain_file() -> FilePath {
        FilePath::dir(r"C:\Program Files\McAfee")
    }

    fn guarded_driver() -> FilePath {
        FilePath::driver(r"C:\Windows\System32\drivers\aswSP.sys", "aswSP")
    }

    fn denied() -> ActionOutcome {
        ActionOutcome::Error("Access is denied.".to_owned())
    }

    fn in_use() -> ActionOutcome {
        ActionOutcome::Error("it is being used by another process".to_owned())
    }

    // ── the happy paths need no force ────────────────────────────────────────

    #[test]
    fn an_already_removed_file_needs_no_forceful_executor() {
        assert_eq!(
            resolve_file(
                &plain_file(),
                &[],
                ActionOutcome::Removed,
                &ScriptedForcefulExecutor::nothing_works()
            ),
            FileResolution::Removed
        );
    }

    #[test]
    fn a_not_found_file_is_already_resolved() {
        assert_eq!(
            resolve_file(
                &plain_file(),
                &[],
                ActionOutcome::NotFound,
                &ScriptedForcefulExecutor::nothing_works()
            ),
            FileResolution::Removed
        );
    }

    // ── denial → ownership ───────────────────────────────────────────────────

    #[test]
    fn a_denied_file_is_removed_once_ownership_is_taken() {
        assert_eq!(
            resolve_file(
                &plain_file(),
                &[],
                denied(),
                &ScriptedForcefulExecutor::ownership_succeeds()
            ),
            FileResolution::Removed
        );
    }

    // ── denial that survives ownership → reboot ──────────────────────────────

    #[test]
    fn a_file_denied_even_after_ownership_is_deferred_to_reboot() {
        let resolution = resolve_file(
            &plain_file(),
            &[],
            denied(),
            &ScriptedForcefulExecutor::only_reboot_works(),
        );
        assert_eq!(
            resolution,
            FileResolution::Deferred(r"C:\Program Files\McAfee".to_owned())
        );
    }

    // ── lock → reboot directly ───────────────────────────────────────────────

    #[test]
    fn a_locked_file_is_deferred_without_trying_ownership() {
        // A recording stub proves ownership is never attempted for a lock.
        struct Recorder {
            ownership_calls: Mutex<usize>,
        }
        impl ForcefulExecutor for Recorder {
            fn take_ownership_and_delete(&self, _: &FilePath) -> ActionOutcome {
                *self.ownership_calls.lock().unwrap() += 1;
                ActionOutcome::Removed
            }
            fn schedule_delete_at_reboot(&self, _: &FilePath) -> ActionOutcome {
                ActionOutcome::Removed
            }
        }

        let recorder = Recorder {
            ownership_calls: Mutex::new(0),
        };
        let resolution = resolve_file(&plain_file(), &[], in_use(), &recorder);

        assert!(matches!(resolution, FileResolution::Deferred(_)));
        assert_eq!(
            *recorder.ownership_calls.lock().unwrap(),
            0,
            "taking ownership cannot free a locked file, so it must be skipped"
        );
    }

    // ── nothing works → failure ──────────────────────────────────────────────

    #[test]
    fn a_file_that_resists_every_remedy_is_reported_failed() {
        let resolution = resolve_file(
            &plain_file(),
            &[],
            denied(),
            &ScriptedForcefulExecutor::nothing_works(),
        );
        assert!(matches!(resolution, FileResolution::Failed(_)));
    }

    // ── the invariant survives the whole loop ────────────────────────────────

    #[test]
    fn a_guarded_driver_is_never_handed_to_the_forceful_executor() {
        // If the ladder tried to force it, this stub would panic.
        struct Forbidden;
        impl ForcefulExecutor for Forbidden {
            fn take_ownership_and_delete(&self, _: &FilePath) -> ActionOutcome {
                panic!("a guarded driver must never reach take-ownership");
            }
            fn schedule_delete_at_reboot(&self, _: &FilePath) -> ActionOutcome {
                panic!("a guarded driver must never be queued for boot deletion");
            }
        }

        // Service still present, and the deletion was denied: the strongest
        // temptation to escalate.
        let resolution = resolve_file(&guarded_driver(), &[], denied(), &Forbidden);
        assert!(matches!(resolution, FileResolution::Skipped(_)));
    }

    #[test]
    fn a_driver_whose_service_is_gone_may_be_forced() {
        let resolution = resolve_file(
            &guarded_driver(),
            &["aswSP"],
            denied(),
            &ScriptedForcefulExecutor::ownership_succeeds(),
        );
        assert_eq!(resolution, FileResolution::Removed);
    }

    // ── takeown / icacls argument construction ───────────────────────────────

    #[test]
    fn taking_ownership_of_a_directory_recurses() {
        let args = takeown_args(&FilePath::dir(r"C:\Program Files\McAfee"));
        assert_eq!(
            args,
            vec!["/F", r"C:\Program Files\McAfee", "/A", "/R", "/D", "Y"]
        );
    }

    #[test]
    fn taking_ownership_of_a_single_file_does_not_recurse() {
        let args = takeown_args(&FilePath::file(r"C:\Windows\System32\drivers\x.sys"));
        assert_eq!(args, vec!["/F", r"C:\Windows\System32\drivers\x.sys", "/A"]);
        assert!(
            !args.iter().any(|a| a == "/R"),
            "a single file must not recurse into anything"
        );
    }

    #[test]
    fn granting_uses_the_administrators_sid_not_a_localized_name() {
        let args = icacls_args(&FilePath::file(r"C:\x\y.sys"));
        assert!(
            args.iter().any(|a| a == "*S-1-5-32-544:(F)"),
            "the well-known SID keeps this working on non-English Windows: {args:?}"
        );
        assert!(
            !args.iter().any(|a| a.contains("Administrators")),
            "the localized name would not match on a non-English system"
        );
    }

    #[test]
    fn granting_on_a_directory_recurses_but_on_a_file_does_not() {
        assert!(
            icacls_args(&FilePath::dir(r"C:\x"))
                .iter()
                .any(|a| a == "/T")
        );
        assert!(
            !icacls_args(&FilePath::file(r"C:\x\y"))
                .iter()
                .any(|a| a == "/T")
        );
    }

    #[test]
    fn the_target_path_is_the_first_icacls_argument() {
        // icacls takes the path first; passing the flags first would target the
        // wrong thing or fail.
        let args = icacls_args(&FilePath::file(r"C:\x\y.sys"));
        assert_eq!(args.first().map(String::as_str), Some(r"C:\x\y.sys"));
    }
}
