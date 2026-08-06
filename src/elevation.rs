//! Administrator privilege detection.
//!
//! Author: PratikP1
//!
//! Every removal action — `reg delete`, `sc delete`, `schtasks /Delete`, and
//! deleting anything under Program Files — requires an elevated token.  The
//! shipped executable carries a manifest that asks Windows to elevate it, but a
//! build produced without that manifest would otherwise fail every single
//! action and present the user with a wall of "Access is denied".  Checking up
//! front lets us say what is actually wrong.

/// `true` when the current process holds an elevated (Administrator) token.
///
/// Always `true` off Windows, where the CLI fallback performs no privileged
/// work and the test suite must run unprivileged.
pub fn is_elevated() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::is_elevated()
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

/// Message shown when Wixen is started without Administrator rights.
pub const NOT_ELEVATED_MESSAGE: &str = concat!(
    "Wixen Uninstaller needs Administrator privileges to remove stubborn software.\n\n",
    "Close this window, then right-click Wixen Uninstaller and choose ",
    "\"Run as administrator\".\n\n",
    "No changes have been made to your system.",
);

#[cfg(target_os = "windows")]
mod windows {
    use std::{ffi::c_void, mem, ptr};

    type Handle = *mut c_void;

    const TOKEN_QUERY: u32 = 0x0008;
    /// `TokenElevation` in the `TOKEN_INFORMATION_CLASS` enumeration.
    const TOKEN_ELEVATION_CLASS: i32 = 20;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(process: Handle, desired_access: u32, token: *mut Handle) -> i32;
        fn GetTokenInformation(
            token: Handle,
            information_class: i32,
            information: *mut c_void,
            information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    pub fn is_elevated() -> bool {
        let Some(token) = open_process_token() else {
            return false;
        };

        let elevated = query_token_elevation(token);

        // SAFETY: `token` came from a successful `OpenProcessToken` and has not
        // been closed yet.
        unsafe { CloseHandle(token) };

        elevated
    }

    fn open_process_token() -> Option<Handle> {
        let mut token: Handle = ptr::null_mut();

        // SAFETY: `GetCurrentProcess` returns a pseudo-handle that is always
        // valid and needs no closing; `token` is a valid out-pointer.
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };

        (opened != 0).then_some(token)
    }

    /// Reads the `TOKEN_ELEVATION` structure, which is a single non-zero
    /// `DWORD` when the token is elevated.
    fn query_token_elevation(token: Handle) -> bool {
        let mut is_elevated: u32 = 0;
        let mut returned_length: u32 = 0;

        // SAFETY: `token` is open with TOKEN_QUERY, and the out-buffer matches
        // the size and layout `TokenElevation` writes.
        let queried = unsafe {
            GetTokenInformation(
                token,
                TOKEN_ELEVATION_CLASS,
                (&raw mut is_elevated).cast::<c_void>(),
                mem::size_of::<u32>() as u32,
                &mut returned_length,
            )
        };

        queried != 0 && is_elevated != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_windows_builds_report_elevated_so_the_cli_can_run() {
        #[cfg(not(target_os = "windows"))]
        assert!(is_elevated());
    }

    #[test]
    fn elevation_message_tells_the_user_what_to_do() {
        assert!(NOT_ELEVATED_MESSAGE.contains("Run as administrator"));
        assert!(NOT_ELEVATED_MESSAGE.contains("No changes have been made"));
    }
}
