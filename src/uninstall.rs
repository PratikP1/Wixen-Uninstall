//! Parsing registry uninstall strings into a runnable command.
//!
//! Author: PratikP1
//!
//! A product's `HKLM\…\Uninstall\<name>` key carries a `QuietUninstallString`
//! or `UninstallString`: a command line, not a path.  Turning it into a program
//! and an argument list is fiddly and security-relevant (an unquoted path with
//! spaces, an MSI product code, a missing silent switch), so it lives here in
//! the pure, tested core.  Nothing in this module executes anything; it returns
//! a [`UninstallCommand`] the Windows layer runs.

use std::fmt;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A parsed, ready-to-run uninstall command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallCommand {
    /// The executable to run, unquoted.
    pub program: String,
    /// Arguments, already split, in order.
    pub args: Vec<String>,
}

/// Why an uninstall string could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallParseError {
    /// The string was empty or all whitespace.
    Empty,
    /// A quoted program path had no closing quote.
    UnterminatedQuote(String),
    /// The program was empty (e.g. `""`), so there is nothing to run.
    MissingProgram(String),
}

impl fmt::Display for UninstallParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UninstallParseError::Empty => f.write_str("uninstall string is empty"),
            UninstallParseError::UnterminatedQuote(string) => {
                write!(f, "unterminated quote in uninstall string {string:?}")
            }
            UninstallParseError::MissingProgram(string) => {
                write!(f, "no program to run in uninstall string {string:?}")
            }
        }
    }
}

impl std::error::Error for UninstallParseError {}

// ─── Public API ──────────────────────────────────────────────────────────────

const MSI_PRODUCT_CODE_START: char = '{';
const MSI_PRODUCT_CODE_END: char = '}';
const MSIEXEC_PROGRAM: &str = "msiexec.exe";

/// Switches that already put an uninstaller into unattended mode.  Matched
/// case-insensitively; if any is present the command is left as the vendor
/// wrote it rather than risk double-passing a flag.
const SILENT_SWITCHES: &[&str] = &[
    "/s",
    "/silent",
    "/verysilent",
    "/quiet",
    "/qn",
    "/qb",
    "/instop:uninstall",
];

impl UninstallCommand {
    /// Parse a registry uninstall string into a runnable command.
    ///
    /// MSI strings (`MsiExec.exe /X{GUID}`) are normalized to a silent
    /// `msiexec /x {GUID} /qn /norestart`.  Other strings are split into
    /// program and arguments, honoring a quoted program path.
    pub fn parse(raw: &str) -> Result<Self, UninstallParseError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(UninstallParseError::Empty);
        }

        if let Some(product_code) = msi_product_code(trimmed) {
            return Ok(Self::silent_msi_uninstall(product_code));
        }

        let (program, rest) = split_program(trimmed)?;
        // A quoted empty program (`""`) or a string of only separators leaves
        // nothing to run; that is an error, not a command with no program.
        if program.is_empty() {
            return Err(UninstallParseError::MissingProgram(trimmed.to_owned()));
        }
        Ok(Self {
            program,
            args: split_arguments(rest),
        })
    }

    /// `true` when the command already runs unattended.
    ///
    /// Wixen runs a vendor uninstaller only when this holds: appending a
    /// *guessed* silent switch could leave the uninstaller blocking on a
    /// dialog, which is precisely the accessibility trap this feature removes.
    /// So the policy is to prefer `QuietUninstallString`, normalize MSI to a
    /// silent form, and otherwise decline to run it, never to guess.
    pub fn is_silent(&self) -> bool {
        self.args.iter().any(|arg| is_known_silent_switch(arg))
    }

    /// `true` when [`program`](Self::program) is a bare filename carrying no
    /// directory: the shape MSI normalization produces (`msiexec.exe`).
    ///
    /// Such a name must never reach the process launcher unchanged while Wixen
    /// is elevated: Windows would resolve it through the search path, which
    /// includes attacker-writable directories, so a planted `msiexec.exe` could
    /// run with full privilege. The Windows runner resolves a bare name to
    /// `System32` instead; a path carrying any separator or drive marker is
    /// already anchored and is left as the vendor wrote it.
    pub fn program_is_bare_name(&self) -> bool {
        !self.program.contains(['\\', '/', ':'])
    }

    fn silent_msi_uninstall(product_code: &str) -> Self {
        Self {
            program: MSIEXEC_PROGRAM.to_owned(),
            args: vec![
                "/x".to_owned(),
                product_code.to_owned(),
                "/qn".to_owned(),
                "/norestart".to_owned(),
            ],
        }
    }
}

// ─── Parsing internals ───────────────────────────────────────────────────────

/// The `{GUID}` from an MSI uninstall string, if this is one.
fn msi_product_code(command: &str) -> Option<&str> {
    let start = command.find(MSI_PRODUCT_CODE_START)?;
    let end = command[start..].find(MSI_PRODUCT_CODE_END)? + start;

    // Only treat it as MSI when the program really is msiexec; a product's own
    // uninstaller can mention a GUID in its path without being an MSI package.
    if !contains_ignore_ascii_case(&command[..start], "msiexec") {
        return None;
    }

    Some(&command[start..=end])
}

/// Split off the program, honoring a quoted path, and return it with the
/// remaining argument text.
fn split_program(command: &str) -> Result<(String, &str), UninstallParseError> {
    if let Some(after_quote) = command.strip_prefix('"') {
        let close = after_quote
            .find('"')
            .ok_or_else(|| UninstallParseError::UnterminatedQuote(command.to_owned()))?;
        let program = after_quote[..close].to_owned();
        let rest = after_quote[close + '"'.len_utf8()..].trim_start();
        Ok((program, rest))
    } else {
        let end = command.find(char::is_whitespace).unwrap_or(command.len());
        Ok((command[..end].to_owned(), command[end..].trim_start()))
    }
}

/// Split an argument string on whitespace, keeping quoted runs together.
fn split_arguments(rest: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;

    for character in rest.chars() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            character if character.is_whitespace() && !in_quotes => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            character => {
                current.push(character);
                has_token = true;
            }
        }
    }

    if has_token {
        args.push(current);
    }

    args
}

fn is_known_silent_switch(arg: &str) -> bool {
    SILENT_SWITCHES
        .iter()
        .any(|silent| arg.eq_ignore_ascii_case(silent))
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    !needle.is_empty()
        && haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── empty ────────────────────────────────────────────────────────────────

    #[test]
    fn an_empty_string_is_rejected() {
        assert_eq!(UninstallCommand::parse(""), Err(UninstallParseError::Empty));
        assert_eq!(
            UninstallCommand::parse("   \t "),
            Err(UninstallParseError::Empty)
        );
    }

    // ── quoted program path ──────────────────────────────────────────────────

    #[test]
    fn a_quoted_program_path_with_spaces_stays_one_token() {
        let command =
            UninstallCommand::parse(r#""C:\Program Files\AVAST Software\Avast\setup\Instup.exe""#)
                .unwrap();
        assert_eq!(
            command.program,
            r"C:\Program Files\AVAST Software\Avast\setup\Instup.exe"
        );
        assert!(command.args.is_empty());
    }

    #[test]
    fn a_quoted_program_keeps_its_following_arguments() {
        let command = UninstallCommand::parse(
            r#""C:\Program Files\AVAST Software\Avast\setup\Instup.exe" /instop:uninstall /silent"#,
        )
        .unwrap();
        assert!(command.program.ends_with("Instup.exe"));
        assert_eq!(command.args, vec!["/instop:uninstall", "/silent"]);
    }

    #[test]
    fn a_quoted_empty_program_is_rejected() {
        // Found by the fuzzer: `""` parsed to a command with an empty program,
        // which would then be handed to the process launcher as the program to
        // run. There must be something to run.
        assert!(matches!(
            UninstallCommand::parse(r#""" /S"#),
            Err(UninstallParseError::MissingProgram(_))
        ));
        assert!(matches!(
            UninstallCommand::parse(r#""""#),
            Err(UninstallParseError::MissingProgram(_))
        ));
    }

    #[test]
    fn an_unterminated_quote_is_rejected() {
        assert!(matches!(
            UninstallCommand::parse(r#""C:\Program Files\X\uninst.exe /S"#),
            Err(UninstallParseError::UnterminatedQuote(_))
        ));
    }

    // ── unquoted program path ────────────────────────────────────────────────

    #[test]
    fn an_unquoted_program_path_splits_at_the_first_space() {
        let command = UninstallCommand::parse(r"C:\PROGRA~1\Norton\uninst.exe /S").unwrap();
        assert_eq!(command.program, r"C:\PROGRA~1\Norton\uninst.exe");
        assert_eq!(command.args, vec!["/S"]);
    }

    #[test]
    fn an_unquoted_program_with_no_arguments_parses() {
        let command = UninstallCommand::parse(r"C:\Windows\uninst.exe").unwrap();
        assert_eq!(command.program, r"C:\Windows\uninst.exe");
        assert!(command.args.is_empty());
    }

    // ── MSI normalization ────────────────────────────────────────────────────

    #[test]
    fn an_msi_uninstall_string_becomes_a_silent_msiexec_call() {
        let command =
            UninstallCommand::parse("MsiExec.exe /X{A7C3D2F1-8E4B-4C9A-B5D6-1F2E3A4B5C6D}")
                .unwrap();
        assert_eq!(command.program, "msiexec.exe");
        assert_eq!(
            command.args,
            vec![
                "/x",
                "{A7C3D2F1-8E4B-4C9A-B5D6-1F2E3A4B5C6D}",
                "/qn",
                "/norestart"
            ]
        );
        assert!(command.is_silent(), "an MSI removal must be unattended");
    }

    #[test]
    fn an_msi_install_string_is_also_normalized_to_an_uninstall() {
        // Some keys store `/I{GUID}`; we always uninstall, never install.
        let command =
            UninstallCommand::parse("msiexec.exe /I{11111111-2222-3333-4444-555555555555}")
                .unwrap();
        assert_eq!(command.args.first().map(String::as_str), Some("/x"));
        assert_eq!(
            command.args.get(1).map(String::as_str),
            Some("{11111111-2222-3333-4444-555555555555}")
        );
    }

    #[test]
    fn a_guid_in_a_non_msiexec_path_is_not_treated_as_msi() {
        // A product folder can contain a GUID without being an MSI package.
        let command =
            UninstallCommand::parse(r#""C:\ProgramData\{5AB}\uninst.exe" /remove"#).unwrap();
        assert_eq!(command.program, r"C:\ProgramData\{5AB}\uninst.exe");
        assert_eq!(command.args, vec!["/remove"]);
    }

    #[test]
    fn a_bare_msiexec_with_no_product_code_is_a_non_silent_passthrough() {
        // The fuzzer found "MSIEXEC.EXE": with no product code it is not an MSI
        // string, so it is passed through as an ordinary program rather than
        // normalized. It carries no silent switch, so `is_silent` is false and
        // the vendor step never runs it, which is why a bare msiexec is safe to
        // parse at face value.
        let command = UninstallCommand::parse("MSIEXEC.EXE").unwrap();
        assert_eq!(command.program, "MSIEXEC.EXE");
        assert!(command.args.is_empty());
        assert!(!command.is_silent(), "a bare msiexec must not be run");
    }

    // ── silence detection ────────────────────────────────────────────────────

    #[test]
    fn a_quiet_string_is_recognized_as_silent() {
        assert!(
            UninstallCommand::parse(r"C:\x\uninst.exe /S")
                .unwrap()
                .is_silent()
        );
        assert!(
            UninstallCommand::parse(r"C:\x\setup.exe /VERYSILENT")
                .unwrap()
                .is_silent()
        );
        assert!(
            UninstallCommand::parse(r"C:\x\Instup.exe /instop:uninstall /silent")
                .unwrap()
                .is_silent()
        );
    }

    #[test]
    fn a_bare_uninstall_string_is_not_silent() {
        assert!(
            !UninstallCommand::parse(r"C:\x\uninst.exe /remove")
                .unwrap()
                .is_silent()
        );
    }

    #[test]
    fn silence_matching_ignores_case() {
        assert!(
            UninstallCommand::parse(r"C:\x\uninst.exe /silent")
                .unwrap()
                .is_silent()
        );
        assert!(
            UninstallCommand::parse(r"C:\x\uninst.exe /SILENT")
                .unwrap()
                .is_silent()
        );
    }

    // ── bare-name detection (search-path safety) ─────────────────────────────

    #[test]
    fn a_normalized_msi_program_is_a_bare_name() {
        // The one shape the parser itself produces unqualified; it must be
        // resolved to System32 before an elevated launch.
        let command =
            UninstallCommand::parse("MsiExec.exe /X{A7C3D2F1-8E4B-4C9A-B5D6-1F2E3A4B5C6D}")
                .unwrap();
        assert_eq!(command.program, "msiexec.exe");
        assert!(command.program_is_bare_name());
    }

    #[test]
    fn any_separator_or_drive_marker_means_not_bare() {
        // Each marker is checked on its own so dropping one from the set fails a
        // test: a backslash, a forward slash, and a bare drive-relative colon.
        for anchored in [
            r"C:\Program Files\X\uninst.exe",
            "dir/uninst.exe",
            r"dir\uninst.exe",
            "C:uninst.exe",
        ] {
            let command = UninstallCommand {
                program: anchored.to_owned(),
                args: Vec::new(),
            };
            assert!(
                !command.program_is_bare_name(),
                "{anchored} is anchored, not a bare name"
            );
        }
    }

    #[test]
    fn a_plain_executable_name_is_bare() {
        let command = UninstallCommand {
            program: "uninst.exe".to_owned(),
            args: Vec::new(),
        };
        assert!(command.program_is_bare_name());
    }

    // ── quoted arguments ─────────────────────────────────────────────────────

    #[test]
    fn a_quoted_argument_keeps_its_internal_spaces() {
        // Without honoring quotes here, a spaced argument path would shatter
        // into several bogus arguments and the uninstaller would misfire.
        let command =
            UninstallCommand::parse(r#"C:\x\uninst.exe "/log:C:\Temp\my log.txt" /S"#).unwrap();
        assert_eq!(command.args, vec![r"/log:C:\Temp\my log.txt", "/S"]);
    }

    // ── error display ────────────────────────────────────────────────────────

    #[test]
    fn errors_describe_themselves() {
        assert!(UninstallParseError::Empty.to_string().contains("empty"));
        let unterminated =
            UninstallParseError::UnterminatedQuote("open-only-quote".to_owned()).to_string();
        assert!(unterminated.contains("unterminated"));
        assert!(
            unterminated.contains("open-only-quote"),
            "the offending input is shown: {unterminated}"
        );

        let missing = UninstallParseError::MissingProgram("just-args".to_owned()).to_string();
        assert!(missing.contains("no program"));
        assert!(missing.contains("just-args"));
    }

    // ── never panics ─────────────────────────────────────────────────────────

    #[test]
    fn odd_but_legal_strings_do_not_panic() {
        for raw in [
            r#""only a quote opens"#,
            "trailing spaces    ",
            r#""" "#,
            "{no msiexec here}",
            "msiexec.exe /X",
            "\"\"",
        ] {
            let _ = UninstallCommand::parse(raw);
        }
    }
}
