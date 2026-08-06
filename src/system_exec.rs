//! Running the whole removal as `NT AUTHORITY\SYSTEM`, headlessly.
//!
//! Author: PratikP1
//!
//! Administrator is not always enough: some services and files are ACL'd
//! against Administrators but not against SYSTEM, and some vendor uninstallers
//! run *truly* silently only under SYSTEM.  So before running the removal
//! itself, the interactive process re-launches Wixen as SYSTEM through a
//! transient scheduled task — the standard technique, needing only the
//! Administrator token Wixen already holds — runs it once, and deletes the task.
//!
//! A SYSTEM process runs in **session 0** with no desktop, so it can show no
//! menu, no progress dialog, and reach no screen reader.  The split is therefore
//! strict: the interactive (Administrator) process owns every screen —
//! selection, confirmation, the waiting dialog, the report — and the SYSTEM
//! process runs headless, executing the plan and writing an
//! [`ExecutionReport`](crate::executor::ExecutionReport) to a results file the
//! interactive process reads back.
//!
//! SYSTEM is an amplifier, never a precondition.  If any step of the relaunch
//! fails, the interactive process runs the removal in-process under
//! Administrator, exactly as it did before this existed.
//!
//! The re-launched instance takes the [`EXECUTE_FLAG`] branch, which only runs
//! the removal and never relaunches — so a SYSTEM run can never spawn another,
//! and no re-entrancy guard is needed.
//!
//! Only the command-line contract, the results path, and the task command are
//! pure and tested here; registering and running the scheduled task is Windows
//! I/O behind `#[cfg(target_os = "windows")]`.

use crate::product::Product;
use std::path::{Path, PathBuf};

// ─── Pure: the command-line contract and where results live ──────────────────

/// The flag that puts a re-launched Wixen into headless SYSTEM-execution mode.
pub const EXECUTE_FLAG: &str = "--execute";

/// The scheduled task Wixen creates, runs once, and deletes.
pub const SYSTEM_TASK_NAME: &str = "WixenSystemUninstall";

/// Sub-directory of `%ProgramData%` holding Wixen's state, matching `reboot`.
const STATE_DIRECTORY: &str = "Wixen";
/// The file the SYSTEM run writes and the interactive process reads.
const RESULTS_FILE: &str = "execute-result.txt";

/// The product to remove headlessly, if `args` request `--execute <slug>`.
///
/// Returns `None` when the flag is absent, carries no slug, or names an unknown
/// product — in every case the normal interactive flow runs instead.  Mirrors
/// [`reboot::is_resume_request`](crate::reboot::is_resume_request), but carries
/// the chosen product across the process boundary.
pub fn parse_execute_request<I, S>(args: I) -> Option<Product>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg.as_ref() == EXECUTE_FLAG {
            return args
                .next()
                .and_then(|slug| Product::from_slug(slug.as_ref()));
        }
    }
    None
}

/// The results file beneath `program_data` (`%ProgramData%`).
pub fn results_file_path(program_data: &Path) -> PathBuf {
    program_data.join(STATE_DIRECTORY).join(RESULTS_FILE)
}

/// The `schtasks /TR` command that runs `executable` headlessly for `product`.
///
/// Quoted so a space in the install path (e.g. `C:\Program Files\…`) does not
/// split the command, mirroring [`reboot::relaunch_command`](crate::reboot::relaunch_command).
pub fn task_run_command(executable: &Path, product: Product) -> String {
    format!(
        "\"{}\" {EXECUTE_FLAG} {}",
        executable.display(),
        product.slug()
    )
}

// ─── The headless removal (runs as SYSTEM; portable, no UI) ───────────────────

/// Run the full removal headlessly and write its report to the results file.
///
/// This is what the [`EXECUTE_FLAG`] branch calls, normally as SYSTEM.  It shows
/// no UI.  Any boot-time resume is registered here, where the privilege is, and
/// the fact recorded so the interactive process can show the restart notice.
///
/// The report is written to a temporary file and then renamed, so the
/// interactive process — which waits for the results file to appear — can never
/// read a half-written report.
pub fn run_and_write_results(product: Product) -> std::io::Result<()> {
    write_results_for(product, &results_file_path(&program_data()))
}

/// Execute `product`'s removal headlessly and write the report to `path`.
///
/// Split from [`run_and_write_results`] so the write can be tested at a
/// caller-chosen path — the public entry point always uses the one fixed
/// `%ProgramData%` location, which parallel tests cannot share safely.
fn write_results_for(product: Product, path: &Path) -> std::io::Result<()> {
    use crate::executor::{LiveExecutor, execute_full};
    use crate::forceful::LiveForcefulExecutor;
    use crate::plan::RemovalPlan;
    use crate::vendor::LiveVendorUninstaller;

    let plan = RemovalPlan::for_product(product);
    let (report, resume) = execute_full(
        &plan,
        &LiveVendorUninstaller,
        &LiveExecutor,
        &LiveForcefulExecutor,
    );
    let resume_registered = match &resume {
        Some(state) => crate::reboot::arrange_resume(state).is_ok(),
        None => false,
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, report.to_results_text(resume_registered))?;
    std::fs::rename(&temporary, path)
}

/// `%ProgramData%`, or its default when the variable is unset.
fn program_data() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(default_program_data)
}

/// `%ProgramData%`'s conventional location when the variable is unset — and,
/// off Windows (where `--execute` is never really used but must still compile
/// and test), the temp directory, so a stray run writes somewhere harmless.
fn default_program_data() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\ProgramData")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::temp_dir()
    }
}

// ─── Windows: registering, running, and awaiting the SYSTEM task ──────────────

/// Re-launch Wixen as SYSTEM to run `product`'s removal, wait for it, and read
/// back its report and whether a resume was registered.
///
/// `on_poll` is called once per wait tick, so the caller can keep a "working"
/// indicator alive.  Returns `Err` if any step fails, which the caller treats
/// as "run it in-process under Administrator instead".
#[cfg(target_os = "windows")]
pub fn run_execution_as_system<F: FnMut()>(
    product: Product,
    mut on_poll: F,
) -> std::io::Result<(crate::executor::ExecutionReport, bool)> {
    windows::run_execution_as_system(product, &mut on_poll)
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{SYSTEM_TASK_NAME, program_data, results_file_path, task_run_command};
    use crate::executor::ExecutionReport;
    use crate::executor::windows::system_tool_path;
    use crate::product::Product;
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// How often to poll for the SYSTEM run's results while it works.
    const POLL_INTERVAL: Duration = Duration::from_millis(500);
    /// How long to wait before giving up and falling back to an in-process run
    /// (fifteen minutes; a literal so no arithmetic invites a stray mutant).
    const MAX_WAIT: Duration = Duration::from_secs(900);

    pub fn run_execution_as_system(
        product: Product,
        on_poll: &mut dyn FnMut(),
    ) -> std::io::Result<(ExecutionReport, bool)> {
        let results = results_file_path(&program_data());
        // Clear any stale result so a previous run's report can never be read as
        // this one's.
        let _ = std::fs::remove_file(&results);
        let _ = std::fs::remove_file(results.with_extension("tmp"));

        let executable = std::env::current_exe()?;
        create_task(&executable, product)?;
        // However this ends — success, error, or panic — remove the task we
        // registered rather than leave it behind.
        let _cleanup = TaskCleanup;
        run_task()?;
        wait_for_results(&results, on_poll)?;

        let text = std::fs::read_to_string(&results)?;
        ExecutionReport::parse_results(&text)
            .ok_or_else(|| std::io::Error::other("the SYSTEM run's results could not be parsed"))
    }

    /// Deletes the transient task on the way out of [`run_execution_as_system`].
    struct TaskCleanup;

    impl Drop for TaskCleanup {
        fn drop(&mut self) {
            let _ = delete_task();
        }
    }

    fn create_task(executable: &Path, product: Product) -> std::io::Result<()> {
        // /RU SYSTEM runs as NT AUTHORITY\SYSTEM without a password (permitted to
        // an Administrator); /RL HIGHEST keeps the elevated token; /SC ONCE with a
        // start time satisfies schtasks, though /Run starts it immediately.
        run_schtasks(&[
            "/Create",
            "/TN",
            SYSTEM_TASK_NAME,
            "/TR",
            &task_run_command(executable, product),
            "/SC",
            "ONCE",
            "/ST",
            "00:00",
            "/RU",
            "SYSTEM",
            "/RL",
            "HIGHEST",
            "/F",
        ])
    }

    fn run_task() -> std::io::Result<()> {
        run_schtasks(&["/Run", "/TN", SYSTEM_TASK_NAME])
    }

    fn delete_task() -> std::io::Result<()> {
        run_schtasks(&["/Delete", "/TN", SYSTEM_TASK_NAME, "/F"])
    }

    fn run_schtasks(args: &[&str]) -> std::io::Result<()> {
        let schtasks = system_tool_path("schtasks.exe")?;
        let status = Command::new(schtasks).args(args).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "schtasks {} exited with {status}",
                args.first().copied().unwrap_or_default()
            )))
        }
    }

    /// Wait for the SYSTEM run to write its results, pulsing `on_poll` so the
    /// interactive process can keep a "working" indicator moving.
    fn wait_for_results(results: &Path, on_poll: &mut dyn FnMut()) -> std::io::Result<()> {
        let start = Instant::now();
        while !results.exists() {
            if start.elapsed() >= MAX_WAIT {
                return Err(std::io::Error::other(
                    "timed out waiting for the SYSTEM run to finish",
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
            on_poll();
        }
        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_execute_request_names_the_product() {
        assert_eq!(
            parse_execute_request(["wixen.exe", "--execute", "avast"]),
            Some(Product::Avast)
        );
        assert_eq!(
            parse_execute_request(["wixen.exe", "--execute", "MCAFEE"]),
            Some(Product::McAfee),
            "the slug reuses from_slug, which is case-insensitive"
        );
    }

    #[test]
    fn a_missing_or_unknown_slug_is_not_an_execute_request() {
        assert_eq!(parse_execute_request(["wixen.exe", "--execute"]), None);
        assert_eq!(
            parse_execute_request(["wixen.exe", "--execute", "kaspersky"]),
            None
        );
    }

    #[test]
    fn without_the_flag_there_is_no_execute_request() {
        assert_eq!(parse_execute_request(["wixen.exe"]), None);
        assert_eq!(parse_execute_request(["wixen.exe", "avast"]), None);
        assert_eq!(parse_execute_request(Vec::<String>::new()), None);
    }

    #[test]
    fn the_run_command_quotes_a_path_with_spaces_and_carries_the_slug() {
        let command = task_run_command(
            Path::new(r"C:\Program Files\Wixen\wixen_uninstall.exe"),
            Product::Avg,
        );
        assert!(
            command.starts_with(r#""C:\Program Files\Wixen\wixen_uninstall.exe""#),
            "an unquoted spaced path would split: {command}"
        );
        assert!(command.contains(EXECUTE_FLAG));
        assert!(
            command.ends_with("avg"),
            "the product slug is passed: {command}"
        );
    }

    #[test]
    fn the_run_command_slug_round_trips_back_to_the_product() {
        // The command the interactive process writes must parse back to the same
        // product in the re-launched instance, or the wrong thing is removed.
        for &product in Product::all() {
            let command = task_run_command(Path::new(r"C:\wixen.exe"), product);
            // The command is `"…exe" --execute <slug>`; the args after the exe
            // are exactly what the re-launched process sees.
            let args = ["--execute", command.rsplit(' ').next().unwrap()];
            assert_eq!(parse_execute_request(args), Some(product));
        }
    }

    #[test]
    fn the_results_file_lives_under_program_data_wixen() {
        let path = results_file_path(Path::new(r"C:\ProgramData"));
        assert!(
            path.ends_with(r"Wixen\execute-result.txt")
                || path.ends_with("Wixen/execute-result.txt")
        );
        assert!(path.starts_with(r"C:\ProgramData"));
    }

    #[test]
    fn writing_results_leaves_a_readable_report() {
        // A run that skips the write leaves the interactive process with nothing
        // to show, so the report must actually reach disk. Off Windows the Live
        // executors are no-ops, so this exercises the plumbing, not a removal.
        //
        // The path is unique per process so parallel test runners — cargo-mutants
        // `--jobs` in particular — never race on one shared results file.
        let dir = std::env::temp_dir().join(format!("wixen_results_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("execute-result.txt");

        write_results_for(Product::McAfee, &path).expect("the report is written");
        let text = std::fs::read_to_string(&path).expect("the report file exists");
        assert!(
            text.contains("product="),
            "the report names its product: {text}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn program_data_is_never_an_empty_path() {
        // An empty base would silently send the results file to a relative path
        // the interactive process would not look in; the lookup always yields the
        // environment value or a real default.
        assert!(!program_data().as_os_str().is_empty());
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn the_default_base_off_windows_is_the_temp_dir() {
        assert_eq!(default_program_data(), std::env::temp_dir());
    }
}
