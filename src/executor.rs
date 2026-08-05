//! Executor — carries out the removal plan.
//!
//! Author: PratikP1
//!
//! On non-Windows targets every action is a no-op so that the test suite
//! compiles and passes on Linux/macOS CI.  On Windows each step is executed
//! for real (requires Administrator privileges).

use crate::plan::{FilePath, RegistryEntry, RemovalPlan, ScheduledTask, ServiceEntry};

// ─── Outcome ─────────────────────────────────────────────────────────────────

/// Result of a single removal action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOutcome {
    Removed,
    NotFound,
    /// Deliberately not attempted, because doing so would have been unsafe.
    Skipped(String),
    Error(String),
}

impl ActionOutcome {
    /// `true` when the artifact is gone: either we removed it, or it was never
    /// there.  Removal is idempotent, so both are wins.
    pub fn is_success(&self) -> bool {
        matches!(self, ActionOutcome::Removed | ActionOutcome::NotFound)
    }
}

/// Aggregated report after executing a full `RemovalPlan`.
#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub product_name: String,
    pub actions_attempted: usize,
    pub actions_succeeded: usize,
    /// Actions refused for safety, with the reason. Not failures, but the
    /// cleanup is incomplete and the user has to be told.
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl ExecutionReport {
    /// `true` only when every action either succeeded or was already done.
    pub fn fully_succeeded(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    pub fn actions_skipped(&self) -> usize {
        self.warnings.len()
    }

    pub fn success_rate(&self) -> f64 {
        if self.actions_attempted == 0 {
            return 1.0;
        }
        self.actions_succeeded as f64 / self.actions_attempted as f64
    }
}

/// Running totals while a plan executes.
#[derive(Default)]
struct Tally {
    succeeded: usize,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl Tally {
    fn record(&mut self, outcome: ActionOutcome, label: &str) {
        match outcome {
            ActionOutcome::Removed | ActionOutcome::NotFound => self.succeeded += 1,
            ActionOutcome::Skipped(reason) => self.warnings.push(format!("{label}: {reason}")),
            ActionOutcome::Error(message) => self.errors.push(format!("{label}: {message}")),
        }
    }
}

#[cfg(any(test, target_os = "windows"))]
fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = stdout.trim();
    let stderr = stderr.trim();

    match (stdout, stderr) {
        ("", "") => String::new(),
        (stdout, "") => stdout.to_owned(),
        ("", stderr) => stderr.to_owned(),
        (stdout, stderr) if stdout == stderr => stdout.to_owned(),
        (stdout, stderr) => format!("{stdout}\n{stderr}"),
    }
}

#[cfg(any(test, target_os = "windows"))]
fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    !needle.is_empty()
        && haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

#[cfg(any(test, target_os = "windows"))]
fn classify_windows_command_result(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
    not_found_markers: &[&str],
) -> ActionOutcome {
    if success {
        return ActionOutcome::Removed;
    }

    let output = command_output_text(stdout, stderr);
    if not_found_markers
        .iter()
        .any(|marker| contains_ascii_case_insensitive(&output, marker))
    {
        ActionOutcome::NotFound
    } else if output.is_empty() {
        ActionOutcome::Error("command failed without output".to_owned())
    } else {
        ActionOutcome::Error(output)
    }
}

// ─── Executor trait ───────────────────────────────────────────────────────────

/// Abstracts the low-level OS calls so that tests can inject a stub.
pub trait Executor {
    fn remove_registry_entry(&self, entry: &RegistryEntry) -> ActionOutcome;
    fn remove_file_path(&self, path: &FilePath) -> ActionOutcome;
    fn stop_and_remove_service(&self, service: &ServiceEntry) -> ActionOutcome;
    fn delete_scheduled_task(&self, task: &ScheduledTask) -> ActionOutcome;
}

// ─── Orchestrator ─────────────────────────────────────────────────────────────

// ─── Progress ────────────────────────────────────────────────────────────────

/// Which group of artifacts the executor is working through.
///
/// Reported to the UI so a long removal can say what it is doing rather than
/// looking frozen — which, with a screen reader, is indistinguishable from a
/// crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalPhase {
    ScheduledTasks,
    Services,
    Files,
    Registry,
}

impl RemovalPhase {
    /// Every phase, in the order [`execute`] runs them.
    pub fn all() -> &'static [RemovalPhase] {
        &[
            RemovalPhase::ScheduledTasks,
            RemovalPhase::Services,
            RemovalPhase::Files,
            RemovalPhase::Registry,
        ]
    }

    /// Position in [`RemovalPhase::all`], so the phase can cross a thread
    /// boundary as a plain integer.
    pub fn index(self) -> usize {
        match self {
            RemovalPhase::ScheduledTasks => 0,
            RemovalPhase::Services => 1,
            RemovalPhase::Files => 2,
            RemovalPhase::Registry => 3,
        }
    }

    /// Present-tense description for the progress display.
    pub fn description(self) -> &'static str {
        match self {
            RemovalPhase::ScheduledTasks => "Removing scheduled tasks…",
            RemovalPhase::Services => "Stopping and removing services…",
            RemovalPhase::Files => "Deleting files and folders…",
            RemovalPhase::Registry => "Cleaning up registry keys…",
        }
    }
}

/// Progress notification: the phase now running, and how many of the plan's
/// actions are finished.
pub type ProgressCallback<'a> = &'a mut dyn FnMut(RemovalPhase, usize);

/// Explains why a driver image was left on disk.
///
/// No Safe Mode: that route is closed to the screen-reader users this tool is
/// for, because Windows 10 Safe Mode does not reliably load the audio driver
/// Narrator needs.  A normal restart often releases the service's hold, so the
/// honest next step is to restart and run Wixen once more.
fn driver_guard_reason(service: &str) -> String {
    format!(
        "left in place because the {service} service is still registered - \
         deleting a registered driver's file can stop Windows from starting. \
         Restart Windows and run Wixen again to finish removing it."
    )
}

/// Execute `plan` using the provided `executor` and return a report.
///
/// Ordering is deliberate and load-bearing.  Scheduled tasks go first so a
/// self-repair task cannot reinstate what we are about to remove; services
/// next, so their file handles are released and — critically — so a driver's
/// registration is gone before its image is deleted; then files; then the
/// registry, which is what makes the removal invisible to Windows.
pub fn execute(plan: &RemovalPlan, executor: &dyn Executor) -> ExecutionReport {
    execute_with_progress(plan, executor, &mut |_, _| {})
}

/// As [`execute`], reporting progress after every action.
///
/// `on_progress` receives the phase now running and the number of actions
/// completed so far, counted against [`RemovalPlan::action_count`].
pub fn execute_with_progress(
    plan: &RemovalPlan,
    executor: &dyn Executor,
    on_progress: ProgressCallback<'_>,
) -> ExecutionReport {
    let mut tally = Tally::default();
    let mut completed = 0usize;
    let mut step = |tally: &mut Tally, outcome, label: &str, phase| {
        tally.record(outcome, label);
        completed += 1;
        on_progress(phase, completed);
    };

    for task in &plan.scheduled_tasks {
        let outcome = executor.delete_scheduled_task(task);
        step(
            &mut tally,
            outcome,
            &task.task_path,
            RemovalPhase::ScheduledTasks,
        );
    }

    let mut removed_services: Vec<&str> = Vec::new();
    for service in &plan.services {
        let outcome = executor.stop_and_remove_service(service);
        if outcome.is_success() {
            removed_services.push(&service.name);
        }
        step(&mut tally, outcome, &service.name, RemovalPhase::Services);
    }

    for file in &plan.file_paths {
        let outcome = match file.blocking_guard(&removed_services) {
            Some(service) => ActionOutcome::Skipped(driver_guard_reason(service)),
            None => executor.remove_file_path(file),
        };
        step(&mut tally, outcome, &file.path, RemovalPhase::Files);
    }

    for entry in &plan.registry_entries {
        let outcome = executor.remove_registry_entry(entry);
        step(&mut tally, outcome, &entry.key_path, RemovalPhase::Registry);
    }

    ExecutionReport {
        product_name: plan.product.display_name().to_owned(),
        actions_attempted: plan.action_count(),
        actions_succeeded: tally.succeeded,
        warnings: tally.warnings,
        errors: tally.errors,
    }
}

// ─── Escalating orchestration ─────────────────────────────────────────────────

use crate::forceful::{FileResolution, ForcefulExecutor, resolve_file};
use crate::resume::ResumeState;
use crate::vendor::{VendorOutcome, VendorUninstaller, run_vendor_uninstallers};

/// A full, escalating removal that does not require Safe Mode.
///
/// Runs the product's own uninstaller first (the route past self-protection),
/// then the standard guarded sweep, escalating each stubborn file — take
/// ownership, or queue for boot-time deletion — via `forceful`.  Returns the
/// report and, when anything was deferred to the next boot, the [`ResumeState`]
/// the caller persists so Wixen can finish after a **normal** restart.
///
/// The ordering and the driver guard are exactly as in [`execute`]; escalation
/// only changes what happens *after* an ordinary deletion is refused, and every
/// such decision routes through the guarded [`resolve_file`].
pub fn execute_full(
    plan: &RemovalPlan,
    vendor: &dyn VendorUninstaller,
    executor: &dyn Executor,
    forceful: &dyn ForcefulExecutor,
) -> (ExecutionReport, Option<ResumeState>) {
    execute_full_with_progress(plan, vendor, executor, forceful, &mut |_, _| {})
}

/// As [`execute_full`], reporting progress after every swept action.
///
/// The vendor uninstaller runs first and reports nothing — it is one opaque,
/// possibly slow step with no per-item progress to give — so the count starts
/// advancing only once the guarded sweep begins.  From there `on_progress`
/// fires after each task, service, file, and registry key, exactly matching
/// [`RemovalPlan::action_count`], so the progress bar the simple
/// [`execute_with_progress`] drives works here unchanged.
pub fn execute_full_with_progress(
    plan: &RemovalPlan,
    vendor: &dyn VendorUninstaller,
    executor: &dyn Executor,
    forceful: &dyn ForcefulExecutor,
    on_progress: ProgressCallback<'_>,
) -> (ExecutionReport, Option<ResumeState>) {
    let mut tally = Tally::default();
    let mut resume = ResumeState::new(plan.product);
    let mut completed = 0usize;

    record_vendor_uninstalls(plan, vendor, &mut tally);

    for task in &plan.scheduled_tasks {
        tally.record(executor.delete_scheduled_task(task), &task.task_path);
        completed += 1;
        on_progress(RemovalPhase::ScheduledTasks, completed);
    }

    let mut removed_services: Vec<&str> = Vec::new();
    for service in &plan.services {
        let outcome = executor.stop_and_remove_service(service);
        if outcome.is_success() {
            removed_services.push(&service.name);
        }
        tally.record(outcome, &service.name);
        completed += 1;
        on_progress(RemovalPhase::Services, completed);
    }

    for file in &plan.file_paths {
        resolve_and_record(
            file,
            &removed_services,
            executor,
            forceful,
            &mut tally,
            &mut resume,
        );
        completed += 1;
        on_progress(RemovalPhase::Files, completed);
    }

    // Registry keys are attempted now.  When a restart is already pending, a key
    // that will not delete yet is more likely to succeed after the product's
    // processes are gone, so it is carried to the resume rather than failed.
    let restart_pending = !resume.pending_files.is_empty();
    for entry in &plan.registry_entries {
        record_registry(entry, executor, restart_pending, &mut tally, &mut resume);
        completed += 1;
        on_progress(RemovalPhase::Registry, completed);
    }

    let report = ExecutionReport {
        product_name: plan.product.display_name().to_owned(),
        actions_attempted: plan.action_count(),
        actions_succeeded: tally.succeeded,
        warnings: tally.warnings,
        errors: tally.errors,
    };
    (report, (!resume.is_empty()).then_some(resume))
}

/// Finish a removal suspended before a restart.
///
/// The files queued for boot-time deletion are already gone (Windows removed
/// them during early boot), so this deletes the registry keys that were waiting
/// on them and confirms the files are absent.  Idempotent: a `NotFound` counts
/// as done, so re-running changes nothing.
pub fn finish_resume(state: &ResumeState, executor: &dyn Executor) -> ExecutionReport {
    let mut tally = Tally::default();

    for path in &state.pending_files {
        // Removed at boot → NotFound now, which is success.  Anything else means
        // the boot-time deletion did not take.
        let file = FilePath::file(path);
        tally.record(executor.remove_file_path(&file), path);
    }

    for key in &state.pending_registry {
        let entry = RegistryEntry::key(key.clone());
        tally.record(executor.remove_registry_entry(&entry), key);
    }

    ExecutionReport {
        product_name: state.product.display_name().to_owned(),
        actions_attempted: state.pending_files.len() + state.pending_registry.len(),
        actions_succeeded: tally.succeeded,
        warnings: tally.warnings,
        errors: tally.errors,
    }
}

/// Run each vendor uninstaller; only a genuine failure to run one is worth the
/// user's attention, so only that becomes a warning.
fn record_vendor_uninstalls(plan: &RemovalPlan, vendor: &dyn VendorUninstaller, tally: &mut Tally) {
    for (key, outcome) in run_vendor_uninstallers(plan.uninstall_keys(), vendor) {
        if let VendorOutcome::Ran(ActionOutcome::Error(message)) = outcome {
            tally
                .warnings
                .push(format!("the {key} uninstaller reported: {message}"));
        }
    }
}

fn resolve_and_record(
    file: &FilePath,
    removed_services: &[&str],
    executor: &dyn Executor,
    forceful: &dyn ForcefulExecutor,
    tally: &mut Tally,
    resume: &mut ResumeState,
) {
    let initial = match file.blocking_guard(removed_services) {
        Some(service) => ActionOutcome::Skipped(driver_guard_reason(service)),
        None => executor.remove_file_path(file),
    };

    match resolve_file(file, removed_services, initial, forceful) {
        FileResolution::Removed => tally.succeeded += 1,
        FileResolution::Skipped(reason) => tally.warnings.push(format!("{}: {reason}", file.path)),
        FileResolution::Deferred(path) => {
            tally
                .warnings
                .push(format!("{path}: will be removed at the next restart"));
            resume.add_pending_file(path);
        }
        FileResolution::Failed(reason) => tally.errors.push(format!("{}: {reason}", file.path)),
    }
}

fn record_registry(
    entry: &RegistryEntry,
    executor: &dyn Executor,
    restart_pending: bool,
    tally: &mut Tally,
    resume: &mut ResumeState,
) {
    let outcome = executor.remove_registry_entry(entry);
    if outcome.is_success() {
        tally.succeeded += 1;
    } else if restart_pending {
        tally.warnings.push(format!(
            "{}: will be cleaned up at the next restart",
            entry.key_path
        ));
        resume.add_pending_registry(entry.key_path.clone());
    } else if let ActionOutcome::Error(message) = outcome {
        tally.errors.push(format!("{}: {message}", entry.key_path));
    }
}

// ─── Live (Windows) executor ──────────────────────────────────────────────────

/// The real executor that talks to the Windows API.
///
/// On non-Windows platforms this type still compiles; every method returns
/// `ActionOutcome::NotFound` so the suite stays green on CI.
pub struct LiveExecutor;

impl Executor for LiveExecutor {
    fn remove_registry_entry(&self, entry: &RegistryEntry) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::delete_registry_entry(entry)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = entry;
            ActionOutcome::NotFound
        }
    }

    fn remove_file_path(&self, path: &FilePath) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::delete_file_path(path)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = path;
            ActionOutcome::NotFound
        }
    }

    fn stop_and_remove_service(&self, service: &ServiceEntry) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::stop_and_delete_service(service)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = service;
            ActionOutcome::NotFound
        }
    }

    fn delete_scheduled_task(&self, task: &ScheduledTask) -> ActionOutcome {
        #[cfg(target_os = "windows")]
        {
            windows::delete_scheduled_task(task)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = task;
            ActionOutcome::NotFound
        }
    }
}

// ─── Windows implementation ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub(crate) mod windows {
    use super::{ActionOutcome, classify_windows_command_result};
    use crate::plan::{FilePath, RegistryEntry, ScheduledTask, ServiceEntry};
    use std::process::Command;

    /// Resolve a system tool by absolute path.  Invoking `reg.exe` by bare name
    /// would search PATH, which an attacker who controls a PATH entry could
    /// use to have this elevated process run their binary instead.
    pub(crate) fn system_tool_path(executable_name: &str) -> std::io::Result<std::path::PathBuf> {
        let system_root = std::env::var_os("SystemRoot").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("SystemRoot is not set; cannot locate {executable_name}"),
            )
        })?;

        Ok(std::path::PathBuf::from(system_root)
            .join("System32")
            .join(executable_name))
    }

    pub fn delete_registry_entry(entry: &RegistryEntry) -> ActionOutcome {
        let reg = match system_tool_path("reg.exe") {
            Ok(path) => path,
            Err(error) => return ActionOutcome::Error(error.to_string()),
        };

        let mut command = Command::new(reg);
        command.args(["delete", &entry.key_path]);
        if let Some(value_name) = &entry.value_name {
            command.args(["/v", value_name]);
        }
        command.arg("/f");

        match command.output() {
            Ok(o) => classify_windows_command_result(
                o.status.success(),
                &o.stdout,
                &o.stderr,
                &[
                    "unable to find the specified registry key or value",
                    "unable to find",
                ],
            ),
            Err(e) => ActionOutcome::Error(e.to_string()),
        }
    }

    pub fn delete_file_path(fp: &FilePath) -> ActionOutcome {
        let p = std::path::Path::new(&fp.path);
        let result = if fp.is_dir {
            std::fs::remove_dir_all(p)
        } else {
            std::fs::remove_file(p)
        };
        match result {
            Ok(()) => ActionOutcome::Removed,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ActionOutcome::NotFound,
            Err(e) => ActionOutcome::Error(e.to_string()),
        }
    }

    pub fn stop_and_delete_service(svc: &ServiceEntry) -> ActionOutcome {
        let sc = match system_tool_path("sc.exe") {
            Ok(path) => path,
            Err(error) => return ActionOutcome::Error(error.to_string()),
        };

        // Stop first (ignore errors — may already be stopped).
        let _ = Command::new(&sc).args(["stop", &svc.name]).output();

        match Command::new(&sc).args(["delete", &svc.name]).output() {
            Ok(o) => classify_windows_command_result(
                o.status.success(),
                &o.stdout,
                &o.stderr,
                &["FAILED 1060", "does not exist as an installed service"],
            ),
            Err(e) => ActionOutcome::Error(e.to_string()),
        }
    }

    pub fn delete_scheduled_task(task: &ScheduledTask) -> ActionOutcome {
        let schtasks = match system_tool_path("schtasks.exe") {
            Ok(path) => path,
            Err(error) => return ActionOutcome::Error(error.to_string()),
        };

        let output = Command::new(schtasks)
            .args(["/Delete", "/TN", &task.task_path, "/F"])
            .output();
        match output {
            Ok(o) => classify_windows_command_result(
                o.status.success(),
                &o.stdout,
                &o.stderr,
                &["0x80070002", "cannot find"],
            ),
            Err(e) => ActionOutcome::Error(e.to_string()),
        }
    }
}

// ─── Stub executor for tests ──────────────────────────────────────────────────

/// Configurable stub used in unit/integration tests.
#[cfg(any(test, feature = "test-utils"))]
pub struct StubExecutor {
    pub registry_outcome: ActionOutcome,
    pub file_outcome: ActionOutcome,
    pub service_outcome: ActionOutcome,
    pub task_outcome: ActionOutcome,
}

#[cfg(any(test, feature = "test-utils"))]
impl StubExecutor {
    pub fn all_removed() -> Self {
        Self {
            registry_outcome: ActionOutcome::Removed,
            file_outcome: ActionOutcome::Removed,
            service_outcome: ActionOutcome::Removed,
            task_outcome: ActionOutcome::Removed,
        }
    }

    pub fn all_not_found() -> Self {
        Self {
            registry_outcome: ActionOutcome::NotFound,
            file_outcome: ActionOutcome::NotFound,
            service_outcome: ActionOutcome::NotFound,
            task_outcome: ActionOutcome::NotFound,
        }
    }

    pub fn all_error(msg: &str) -> Self {
        Self {
            registry_outcome: ActionOutcome::Error(msg.to_owned()),
            file_outcome: ActionOutcome::Error(msg.to_owned()),
            service_outcome: ActionOutcome::Error(msg.to_owned()),
            task_outcome: ActionOutcome::Error(msg.to_owned()),
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Executor for StubExecutor {
    fn remove_registry_entry(&self, _: &RegistryEntry) -> ActionOutcome {
        self.registry_outcome.clone()
    }
    fn remove_file_path(&self, _: &FilePath) -> ActionOutcome {
        self.file_outcome.clone()
    }
    fn stop_and_remove_service(&self, _: &ServiceEntry) -> ActionOutcome {
        self.service_outcome.clone()
    }
    fn delete_scheduled_task(&self, _: &ScheduledTask) -> ActionOutcome {
        self.task_outcome.clone()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forceful::ScriptedForcefulExecutor;
    use crate::plan::RemovalPlan;
    use crate::product::Product;
    use crate::vendor::ScriptedVendorUninstaller;
    use std::sync::Mutex;

    fn mcafee_plan() -> RemovalPlan {
        RemovalPlan::for_product(Product::McAfee)
    }

    fn norton_plan() -> RemovalPlan {
        RemovalPlan::for_product(Product::Norton)
    }

    // ── ActionOutcome helpers ─────────────────────────────────────────────────

    #[test]
    fn removed_is_success() {
        assert!(ActionOutcome::Removed.is_success());
    }

    #[test]
    fn command_output_text_prefers_non_empty_stream() {
        assert_eq!(
            command_output_text(b"ok", b""),
            "ok",
            "stdout-only output should be preserved"
        );
        assert_eq!(
            command_output_text(b"", b"fail"),
            "fail",
            "stderr-only output should be preserved"
        );
    }

    #[test]
    fn command_output_text_combines_distinct_streams() {
        assert_eq!(command_output_text(b"one", b"two"), "one\ntwo");
        assert_eq!(command_output_text(b"same", b"same"), "same");
    }

    #[test]
    fn classify_windows_command_result_detects_not_found_in_stdout() {
        assert_eq!(
            classify_windows_command_result(
                false,
                b"[SC] OpenService FAILED 1060:",
                b"",
                &["FAILED 1060"],
            ),
            ActionOutcome::NotFound
        );
    }

    #[test]
    fn classify_windows_command_result_detects_not_found_in_stderr() {
        assert_eq!(
            classify_windows_command_result(
                false,
                b"",
                b"ERROR: The system was unable to find the specified registry key or value.",
                &["unable to find the specified registry key or value"],
            ),
            ActionOutcome::NotFound
        );
    }

    #[test]
    fn classify_windows_command_result_returns_error_when_marker_missing() {
        assert_eq!(
            classify_windows_command_result(false, b"", b"Access is denied.", &["FAILED 1060"]),
            ActionOutcome::Error("Access is denied.".into())
        );
    }

    #[test]
    fn not_found_is_success() {
        assert!(ActionOutcome::NotFound.is_success());
    }

    #[test]
    fn error_is_not_success() {
        assert!(!ActionOutcome::Error("oops".into()).is_success());
    }

    #[test]
    fn skipped_is_not_success() {
        assert!(!ActionOutcome::Skipped("guarded".into()).is_success());
    }

    #[test]
    fn actions_skipped_counts_every_warning() {
        let mut report = ExecutionReport {
            product_name: "test".to_owned(),
            actions_attempted: 3,
            actions_succeeded: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        assert_eq!(report.actions_skipped(), 0);

        report.warnings.push("one".to_owned());
        assert_eq!(report.actions_skipped(), 1);

        report.warnings.push("two".to_owned());
        assert_eq!(report.actions_skipped(), 2);
    }

    #[test]
    fn success_rate_is_one_for_an_empty_plan() {
        let report = ExecutionReport {
            product_name: "test".to_owned(),
            actions_attempted: 0,
            actions_succeeded: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        assert_eq!(report.success_rate(), 1.0);
    }

    // ── Driver guard: the boot-safety invariant ──────────────────────────────

    /// Executor that fails only the named service, so we can reproduce the
    /// real-world case: self-protection blocks `sc delete`, and deleting the
    /// driver image anyway would leave Windows unable to boot.
    struct ServiceFailsExecutor {
        failing_service: &'static str,
        deleted_files: Mutex<Vec<String>>,
    }

    impl Executor for ServiceFailsExecutor {
        fn remove_registry_entry(&self, _: &RegistryEntry) -> ActionOutcome {
            ActionOutcome::Removed
        }

        fn remove_file_path(&self, path: &FilePath) -> ActionOutcome {
            self.deleted_files.lock().unwrap().push(path.path.clone());
            ActionOutcome::Removed
        }

        fn stop_and_remove_service(&self, service: &ServiceEntry) -> ActionOutcome {
            if service.name == self.failing_service {
                ActionOutcome::Error("Access is denied".to_owned())
            } else {
                ActionOutcome::Removed
            }
        }

        fn delete_scheduled_task(&self, _: &ScheduledTask) -> ActionOutcome {
            ActionOutcome::Removed
        }
    }

    #[test]
    fn driver_image_is_not_deleted_when_its_service_survives() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let executor = ServiceFailsExecutor {
            failing_service: "aswSP",
            deleted_files: Mutex::new(Vec::new()),
        };

        let report = execute(&plan, &executor);
        let deleted = executor.deleted_files.lock().unwrap().clone();

        assert!(
            !deleted.iter().any(|path| path.ends_with("aswSP.sys")),
            "the image of a still-registered driver must survive: {deleted:?}"
        );
        assert!(
            deleted.iter().any(|path| path.ends_with("aswSnx.sys")),
            "drivers whose service was removed should still be deleted: {deleted:?}"
        );
        assert!(
            report.warnings.iter().any(|w| w.contains("aswSP.sys")),
            "the skipped driver must be reported: {:?}",
            report.warnings
        );
    }

    #[test]
    fn every_driver_is_deleted_when_all_services_are_removed() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let executor = ServiceFailsExecutor {
            failing_service: "no-such-service",
            deleted_files: Mutex::new(Vec::new()),
        };

        let report = execute(&plan, &executor);

        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(
            executor.deleted_files.lock().unwrap().len(),
            plan.file_paths.len()
        );
    }

    #[test]
    fn skipped_actions_are_warnings_not_errors() {
        let plan = RemovalPlan::for_product(Product::Avg);
        let executor = ServiceFailsExecutor {
            failing_service: "avgSP",
            deleted_files: Mutex::new(Vec::new()),
        };

        let report = execute(&plan, &executor);

        assert_eq!(report.actions_skipped(), 1);
        assert_eq!(report.errors.len(), 1, "only the service itself failed");
        assert!(!report.fully_succeeded());
    }

    #[test]
    fn guard_reason_explains_the_boot_risk_and_the_fix() {
        let reason = driver_guard_reason("aswSP");
        assert!(reason.contains("aswSP"));
        assert!(reason.contains("Windows from starting"));
        assert!(
            reason.contains("Restart Windows"),
            "the fix is a normal restart, not Safe Mode: {reason}"
        );
        assert!(
            !reason.contains("Safe Mode"),
            "Safe Mode has no audio for a screen reader, so it must never be the advice: {reason}"
        );
    }

    // ── ExecutionReport helpers ───────────────────────────────────────────────

    #[test]
    fn report_fully_succeeded_when_no_errors() {
        let plan = mcafee_plan();
        let stub = StubExecutor::all_removed();
        let report = execute(&plan, &stub);
        assert!(report.fully_succeeded());
    }

    #[test]
    fn report_not_fully_succeeded_when_errors_present() {
        let plan = mcafee_plan();
        let stub = StubExecutor::all_error("access denied");
        let report = execute(&plan, &stub);
        assert!(!report.fully_succeeded());
    }

    #[test]
    fn success_rate_is_one_when_all_removed() {
        let plan = norton_plan();
        let stub = StubExecutor::all_removed();
        let report = execute(&plan, &stub);
        assert!((report.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn success_rate_is_zero_when_all_error() {
        let plan = norton_plan();
        let stub = StubExecutor::all_error("fail");
        let report = execute(&plan, &stub);
        assert_eq!(report.success_rate(), 0.0);
    }

    // ── execute orchestration ─────────────────────────────────────────────────

    #[test]
    fn actions_attempted_matches_plan_action_count() {
        let plan = mcafee_plan();
        let count = plan.action_count();
        let stub = StubExecutor::all_removed();
        let report = execute(&plan, &stub);
        assert_eq!(report.actions_attempted, count);
    }

    #[test]
    fn actions_succeeded_equals_attempted_when_all_not_found() {
        // NotFound is considered success (idempotent removal).
        let plan = mcafee_plan();
        let stub = StubExecutor::all_not_found();
        let report = execute(&plan, &stub);
        assert_eq!(report.actions_succeeded, report.actions_attempted);
    }

    #[test]
    fn errors_contain_action_label_when_error_occurs() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        let stub = StubExecutor::all_error("permission denied");
        let report = execute(&plan, &stub);
        assert!(!report.errors.is_empty());
        // Every error message should carry the failing label.
        for err in &report.errors {
            assert!(
                err.contains("permission denied"),
                "Expected 'permission denied' in: {err}"
            );
        }
    }

    #[test]
    fn report_product_name_matches_product() {
        let plan = norton_plan();
        let stub = StubExecutor::all_removed();
        let report = execute(&plan, &stub);
        assert!(report.product_name.contains("Norton"));
    }

    #[test]
    fn success_rate_between_zero_and_one() {
        let plan = mcafee_plan();
        let stub = StubExecutor::all_removed();
        let report = execute(&plan, &stub);
        assert!((0.0..=1.0).contains(&report.success_rate()));
    }

    struct RecordingExecutor {
        calls: Mutex<Vec<&'static str>>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn sequence(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Executor for RecordingExecutor {
        fn remove_registry_entry(&self, _: &RegistryEntry) -> ActionOutcome {
            self.calls.lock().unwrap().push("registry");
            ActionOutcome::Removed
        }

        fn remove_file_path(&self, _: &FilePath) -> ActionOutcome {
            self.calls.lock().unwrap().push("file");
            ActionOutcome::Removed
        }

        fn stop_and_remove_service(&self, _: &ServiceEntry) -> ActionOutcome {
            self.calls.lock().unwrap().push("service");
            ActionOutcome::Removed
        }

        fn delete_scheduled_task(&self, _: &ScheduledTask) -> ActionOutcome {
            self.calls.lock().unwrap().push("task");
            ActionOutcome::Removed
        }
    }

    // ── execute_full: the escalating orchestration ───────────────────────────

    #[test]
    fn a_full_run_with_everything_removable_needs_no_restart() {
        let plan = mcafee_plan();
        let (report, resume) = execute_full(
            &plan,
            &ScriptedVendorUninstaller::nothing_registered(),
            &StubExecutor::all_removed(),
            &ScriptedForcefulExecutor::ownership_succeeds(),
        );

        assert!(report.fully_succeeded(), "{:?}", report.errors);
        assert_eq!(report.actions_succeeded, plan.action_count());
        assert!(
            resume.is_none(),
            "nothing was deferred, so no restart is needed"
        );
    }

    #[test]
    fn a_locked_file_defers_to_a_restart_and_populates_the_resume_state() {
        // Files are denied; ownership fails; only the reboot queue accepts them.
        let plan = RemovalPlan::for_product(Product::Avast);
        let (report, resume) = execute_full(
            &plan,
            &ScriptedVendorUninstaller::nothing_registered(),
            &StubExecutor::all_error("Access is denied"),
            &ScriptedForcefulExecutor::only_reboot_works(),
        );

        let resume = resume.expect("a restart is pending, so a resume state is returned");
        // Only the non-guarded files defer: the drivers stay guarded because
        // every service removal failed, so they are skipped, never queued. That
        // the counts differ is the guard working end to end.
        let deferrable = plan
            .file_paths
            .iter()
            .filter(|file| file.guard_service.is_none())
            .count();
        assert_eq!(
            resume.pending_files.len(),
            deferrable,
            "every non-guarded denied file the queue accepted is pending"
        );
        assert!(
            deferrable < plan.file_paths.len(),
            "some drivers should stay guarded"
        );
        assert!(
            !report.fully_succeeded(),
            "a pending restart is not full success"
        );
        // The registry keys, failing while a restart is pending, are carried too.
        assert!(!resume.pending_registry.is_empty());
    }

    #[test]
    fn a_guarded_driver_is_never_deferred_while_its_service_survives() {
        // Every service removal fails, so every driver stays guarded; a guarded
        // driver must be skipped, never queued for boot deletion.
        let plan = RemovalPlan::for_product(Product::Avast);
        let (_report, resume) = execute_full(
            &plan,
            &ScriptedVendorUninstaller::nothing_registered(),
            &StubExecutor::all_error("Access is denied"),
            &ScriptedForcefulExecutor::only_reboot_works(),
        );

        let deferred = resume.map(|state| state.pending_files).unwrap_or_default();
        for driver in plan
            .file_paths
            .iter()
            .filter(|file| file.guard_service.is_some())
        {
            assert!(
                !deferred.contains(&driver.path),
                "guarded driver {} must not be queued for boot deletion",
                driver.path
            );
        }
    }

    #[test]
    fn a_silent_vendor_uninstaller_is_run_during_a_full_removal() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let uninstall_key = plan.uninstall_keys().next().expect("Avast has one");
        let vendor = ScriptedVendorUninstaller::with_string(
            uninstall_key,
            r#""C:\…\Instup.exe" /instop:uninstall /silent"#,
            ActionOutcome::Removed,
        );

        let _ = execute_full(
            &plan,
            &vendor,
            &StubExecutor::all_removed(),
            &ScriptedForcefulExecutor::ownership_succeeds(),
        );

        assert_eq!(
            vendor.commands_run().len(),
            1,
            "the registered silent uninstaller should have been run"
        );
    }

    #[test]
    fn a_full_run_reports_progress_through_every_phase_up_to_the_action_count() {
        // The Windows progress dialog looks the phase up by index and draws the
        // bar from `completed / action_count`; if either drifts the dialog lies
        // or freezes, which a screen-reader user cannot tell from a crash.
        let plan = RemovalPlan::for_product(Product::Avast);
        let mut phases = Vec::new();
        let mut last_completed = 0;

        let (report, _resume) = execute_full_with_progress(
            &plan,
            &ScriptedVendorUninstaller::nothing_registered(),
            &StubExecutor::all_removed(),
            &ScriptedForcefulExecutor::ownership_succeeds(),
            &mut |phase, completed| {
                if phases.last() != Some(&phase) {
                    phases.push(phase);
                }
                assert!(completed > last_completed, "the count must only ever rise");
                last_completed = completed;
            },
        );

        assert_eq!(
            phases,
            vec![
                RemovalPhase::ScheduledTasks,
                RemovalPhase::Services,
                RemovalPhase::Files,
                RemovalPhase::Registry,
            ],
            "the sweep must report its phases in execution order"
        );
        assert_eq!(
            last_completed,
            plan.action_count(),
            "progress must reach exactly the action count"
        );
        assert_eq!(last_completed, report.actions_attempted);
    }

    #[test]
    fn a_registry_failure_with_nothing_deferred_is_an_error_not_a_restart() {
        // Files all delete, so nothing is queued for reboot; a registry key that
        // then fails has no pending restart to ride on, so it must be a hard
        // error — not silently carried to a resume that will never happen.
        let plan = mcafee_plan();
        let stub = StubExecutor {
            registry_outcome: ActionOutcome::Error("still locked".to_owned()),
            file_outcome: ActionOutcome::Removed,
            service_outcome: ActionOutcome::Removed,
            task_outcome: ActionOutcome::Removed,
        };

        let (report, resume) = execute_full(
            &plan,
            &ScriptedVendorUninstaller::nothing_registered(),
            &stub,
            &ScriptedForcefulExecutor::ownership_succeeds(),
        );

        assert!(
            resume.is_none(),
            "no file was deferred, so there is nothing to resume"
        );
        assert!(
            report.errors.iter().any(|e| e.contains("still locked")),
            "the registry failure must surface as an error: {:?}",
            report.errors
        );
    }

    #[test]
    fn a_failed_vendor_uninstaller_is_a_warning_not_a_dead_end() {
        let plan = RemovalPlan::for_product(Product::Avast);
        let uninstall_key = plan.uninstall_keys().next().unwrap();
        let vendor = ScriptedVendorUninstaller::with_string(
            uninstall_key,
            r"C:\x\uninst.exe /S",
            ActionOutcome::Error("vendor uninstaller crashed".to_owned()),
        );

        // The sweep still removes everything, so despite the vendor failure the
        // run completes; the failure is surfaced as a warning.
        let (report, _resume) = execute_full(
            &plan,
            &vendor,
            &StubExecutor::all_removed(),
            &ScriptedForcefulExecutor::ownership_succeeds(),
        );

        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("vendor uninstaller crashed")),
            "the vendor failure should be reported: {:?}",
            report.warnings
        );
    }

    // ── finish_resume: completing after a restart ────────────────────────────

    #[test]
    fn finishing_a_resume_deletes_the_pending_registry_and_confirms_the_files() {
        let mut state = crate::resume::ResumeState::new(Product::Avast);
        state.add_pending_file(r"C:\Program Files\AVAST Software");
        state.add_pending_registry(r"HKLM\SOFTWARE\AVAST Software");

        // The file was deleted at boot (NotFound now) and the key deletes: all
        // done.
        let report = finish_resume(&state, &StubExecutor::all_not_found());

        assert!(report.fully_succeeded(), "{:?}", report.errors);
        assert_eq!(report.actions_attempted, 2);
        assert!(report.product_name.contains("Avast"));
    }

    #[test]
    fn a_resume_whose_registry_still_fails_reports_the_error() {
        let mut state = crate::resume::ResumeState::new(Product::McAfee);
        state.add_pending_registry(r"HKLM\SOFTWARE\McAfee");

        let report = finish_resume(&state, &StubExecutor::all_error("still locked"));

        assert!(!report.fully_succeeded());
        assert!(report.errors.iter().any(|e| e.contains("still locked")));
    }

    #[test]
    fn execute_removes_tasks_before_services_files_and_registry() {
        let plan = RemovalPlan {
            product: Product::Avast,
            registry_entries: vec![RegistryEntry::key(r"HKLM\SOFTWARE\AVAST Software")],
            file_paths: vec![FilePath::dir(r"C:\ProgramData\AVAST Software")],
            services: vec![ServiceEntry::new("AvastSvc")],
            scheduled_tasks: vec![ScheduledTask::new(r"\AVAST Software\Avast\Overseer")],
        };
        // Services must precede files so a driver's registration is gone before
        // its image is; the assertion below is what keeps that ordering honest.
        let executor = RecordingExecutor::new();

        let report = execute(&plan, &executor);

        assert!(report.fully_succeeded());
        assert_eq!(
            executor.sequence(),
            vec!["task", "service", "file", "registry"]
        );
    }
}
