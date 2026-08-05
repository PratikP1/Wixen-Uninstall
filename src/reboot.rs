//! Persisting a suspended removal and arranging to finish it after a restart.
//!
//! Author: PratikP1
//!
//! When a locked file is queued for boot-time deletion, the removal is
//! suspended.  Wixen writes the [`ResumeState`] to a file under
//! `%ProgramData%\Wixen\` and registers a `RunOnce` entry so Windows launches
//! it once more after the **next normal restart** — audio present, screen
//! reader running.  It then reads the state back, finishes the registry cleanup,
//! reports, and clears both the file and the `RunOnce` entry.
//!
//! The path derivation and the relaunch command are pure and tested here; the
//! file and registry I/O live behind `#[cfg(target_os = "windows")]`.

use crate::resume::ResumeState;
use std::path::{Path, PathBuf};

// ─── Pure: where state lives, and how Wixen relaunches ───────────────────────

/// Sub-directory of `%ProgramData%` holding Wixen's state.
const STATE_DIRECTORY: &str = "Wixen";
/// The suspended-removal state file.
const STATE_FILE: &str = "resume.txt";
/// The command-line flag that tells a fresh Wixen to finish a suspended run.
pub const RESUME_FLAG: &str = "--resume";
/// The `RunOnce` value name under which the relaunch is registered.
pub const RUNONCE_VALUE_NAME: &str = "WixenUninstaller";

/// The resume-state file path beneath `program_data` (`%ProgramData%`).
pub fn state_file_path(program_data: &Path) -> PathBuf {
    program_data.join(STATE_DIRECTORY).join(STATE_FILE)
}

/// The `RunOnce` command that relaunches `executable` to finish the removal.
///
/// Quoted so a space in the install path (e.g. `C:\Program Files\…`) does not
/// split the command Windows runs at next logon.
pub fn relaunch_command(executable: &Path) -> String {
    format!("\"{}\" {RESUME_FLAG}", executable.display())
}

/// `true` when `args` (typically `std::env::args()`) requests resume mode.
pub fn is_resume_request<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == RESUME_FLAG)
}

// ─── Windows: the actual persistence ─────────────────────────────────────────

/// Persist `state` and register the `RunOnce` relaunch, so the removal resumes
/// after the next restart.  Off Windows this is a no-op.
#[cfg(target_os = "windows")]
pub fn arrange_resume(state: &ResumeState) -> std::io::Result<()> {
    windows::arrange_resume(state)
}

/// Read a suspended [`ResumeState`], if one was left by a previous run.
#[cfg(target_os = "windows")]
pub fn take_pending_resume() -> Option<ResumeState> {
    windows::take_pending_resume()
}

#[cfg(not(target_os = "windows"))]
pub fn arrange_resume(state: &ResumeState) -> std::io::Result<()> {
    let _ = state;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn take_pending_resume() -> Option<ResumeState> {
    None
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{
        RUNONCE_VALUE_NAME, ResumeState, STATE_DIRECTORY, STATE_FILE, relaunch_command,
        state_file_path,
    };
    use crate::executor::windows::system_tool_path;
    use std::{path::PathBuf, process::Command};

    const RUNONCE_KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce";

    fn program_data() -> PathBuf {
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
    }

    pub fn arrange_resume(state: &ResumeState) -> std::io::Result<()> {
        let directory = program_data().join(STATE_DIRECTORY);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join(STATE_FILE), state.to_text())?;
        register_runonce()
    }

    fn register_runonce() -> std::io::Result<()> {
        let executable = std::env::current_exe()?;
        let reg = system_tool_path("reg.exe")?;
        Command::new(reg)
            .args([
                "add",
                RUNONCE_KEY,
                "/v",
                RUNONCE_VALUE_NAME,
                "/t",
                "REG_SZ",
                "/d",
                &relaunch_command(&executable),
                "/f",
            ])
            .status()
            .map(|_| ())
    }

    pub fn take_pending_resume() -> Option<ResumeState> {
        let path = state_file_path(&program_data());
        let text = std::fs::read_to_string(&path).ok()?;
        // Best-effort cleanup: the state is consumed once, whatever it holds.
        let _ = std::fs::remove_file(&path);
        let _ = clear_runonce();
        ResumeState::parse(&text)
    }

    fn clear_runonce() -> std::io::Result<()> {
        let reg = system_tool_path("reg.exe")?;
        Command::new(reg)
            .args(["delete", RUNONCE_KEY, "/v", RUNONCE_VALUE_NAME, "/f"])
            .status()
            .map(|_| ())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_file_lives_under_program_data_wixen() {
        let path = state_file_path(Path::new(r"C:\ProgramData"));
        assert!(path.ends_with(r"Wixen\resume.txt") || path.ends_with("Wixen/resume.txt"));
        assert!(path.starts_with(r"C:\ProgramData"));
    }

    #[test]
    fn the_state_file_follows_a_relocated_program_data() {
        let path = state_file_path(Path::new(r"D:\Data"));
        assert!(path.starts_with(r"D:\Data"));
    }

    #[test]
    fn the_relaunch_command_quotes_a_path_with_spaces() {
        let command = relaunch_command(Path::new(r"C:\Program Files\Wixen\wixen_uninstall.exe"));
        assert!(
            command.starts_with(r#""C:\Program Files\Wixen\wixen_uninstall.exe""#),
            "an unquoted spaced path would split: {command}"
        );
        assert!(command.ends_with("--resume"));
    }

    #[test]
    fn the_resume_flag_is_recognized_only_when_present() {
        assert!(is_resume_request(["wixen.exe", "--resume"]));
        assert!(!is_resume_request(["wixen.exe"]));
        assert!(!is_resume_request(["wixen.exe", "--resumexyz"]));
        assert!(!is_resume_request(Vec::<String>::new()));
    }
}
