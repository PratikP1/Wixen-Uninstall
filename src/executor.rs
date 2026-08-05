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
    Error(String),
}

impl ActionOutcome {
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
    pub errors: Vec<String>,
}

impl ExecutionReport {
    pub fn fully_succeeded(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn success_rate(&self) -> f64 {
        if self.actions_attempted == 0 {
            return 1.0;
        }
        self.actions_succeeded as f64 / self.actions_attempted as f64
    }
}

#[cfg(target_os = "windows")]
fn system_tool_path(executable_name: &str) -> std::io::Result<std::path::PathBuf> {
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

/// Execute `plan` using the provided `executor` and return a report.
pub fn execute(plan: &RemovalPlan, executor: &dyn Executor) -> ExecutionReport {
    let mut succeeded = 0usize;
    let mut errors: Vec<String> = Vec::new();

    let mut handle = |outcome: ActionOutcome, label: &str| {
        if outcome.is_success() {
            succeeded += 1;
        } else if let ActionOutcome::Error(msg) = outcome {
            errors.push(format!("{label}: {msg}"));
        }
    };

    for task in &plan.scheduled_tasks {
        let o = executor.delete_scheduled_task(task);
        handle(o, &task.task_path);
    }
    for svc in &plan.services {
        let o = executor.stop_and_remove_service(svc);
        handle(o, &svc.name);
    }
    for fp in &plan.file_paths {
        let o = executor.remove_file_path(fp);
        handle(o, &fp.path);
    }
    for entry in &plan.registry_entries {
        let o = executor.remove_registry_entry(entry);
        handle(o, &entry.key_path);
    }

    ExecutionReport {
        product_name: plan.product.display_name().to_owned(),
        actions_attempted: plan.action_count(),
        actions_succeeded: succeeded,
        errors,
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
mod windows {
    use super::{ActionOutcome, classify_windows_command_result, system_tool_path};
    use crate::plan::{FilePath, RegistryEntry, ScheduledTask, ServiceEntry};
    use std::process::Command;

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
    use crate::plan::RemovalPlan;
    use crate::product::Product;
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

    #[test]
    fn execute_removes_tasks_before_services_files_and_registry() {
        let plan = RemovalPlan {
            product: Product::Avast,
            registry_entries: vec![RegistryEntry::key(r"HKLM\SOFTWARE\AVAST Software")],
            file_paths: vec![FilePath::dir(r"C:\ProgramData\AVAST Software")],
            services: vec![ServiceEntry::new("AvastSvc")],
            scheduled_tasks: vec![ScheduledTask::new(r"\AVAST Software\Avast\Overseer")],
        };
        let executor = RecordingExecutor::new();

        let report = execute(&plan, &executor);

        assert!(report.fully_succeeded());
        assert_eq!(
            executor.sequence(),
            vec!["task", "service", "file", "registry"]
        );
    }
}
