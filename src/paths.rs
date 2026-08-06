//! Windows location resolution and delete-target safety checks.
//!
//! Author: PratikP1
//!
//! Removal plans are written against placeholders such as `{ProgramFiles}`
//! rather than a hard-coded `C:\`.  Windows is not always installed on C:, and
//! `ProgramData` can be relocated, so a hard-coded root silently cleans nothing
//! while still reporting success.  Placeholders are expanded once, when the
//! plan is built.
//!
//! Every expanded path is then checked against [`WindowsLocations::validate`]
//! before it can reach the executor.  Wixen deletes directories recursively, so
//! a template bug that produced `C:\` or `C:\Windows\System32` would be
//! catastrophic; validation makes that unrepresentable rather than unlikely.

use std::fmt;

// ─── Placeholders ────────────────────────────────────────────────────────────

pub const SYSTEM_DRIVE: &str = "SystemDrive";
pub const PROGRAM_FILES: &str = "ProgramFiles";
pub const PROGRAM_FILES_X86: &str = "ProgramFilesX86";
pub const PROGRAM_DATA: &str = "ProgramData";
pub const SYSTEM_ROOT: &str = "SystemRoot";

const PLACEHOLDER_OPEN: char = '{';
const PLACEHOLDER_CLOSE: char = '}';
const SEPARATOR: char = '\\';

/// A path directly beneath a drive root (`C:\McAfee`) is never something we
/// are willing to delete recursively; real product data always nests deeper.
const MINIMUM_SEGMENTS_BELOW_DRIVE: usize = 2;

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Why a path template could not be turned into a safe, absolute target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The template referenced a placeholder this resolver does not define.
    UnknownPlaceholder(String),
    /// The template contained an unterminated `{`.
    UnterminatedPlaceholder(String),
    /// The expansion is not an absolute `X:\…` Windows path.
    NotAbsolute(String),
    /// The expansion contains an empty, `.`, or `..` segment.
    MalformedSegment(String),
    /// The expansion names a system location that must never be deleted.
    ProtectedLocation(String),
    /// The expansion sits too close to the drive root to be a product folder.
    TooShallow(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::UnknownPlaceholder(name) => write!(f, "unknown placeholder {{{name}}}"),
            PathError::UnterminatedPlaceholder(template) => {
                write!(f, "unterminated placeholder in {template}")
            }
            PathError::NotAbsolute(path) => write!(f, "{path} is not an absolute Windows path"),
            PathError::MalformedSegment(path) => {
                write!(f, "{path} has an empty or relative segment")
            }
            PathError::ProtectedLocation(path) => {
                write!(f, "{path} is a protected system location")
            }
            PathError::TooShallow(path) => write!(f, "{path} is too close to the drive root"),
        }
    }
}

impl std::error::Error for PathError {}

// ─── Resolver ────────────────────────────────────────────────────────────────

/// The machine-specific directories a removal plan is written against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsLocations {
    system_drive: String,
    program_files: String,
    program_files_x86: String,
    program_data: String,
    system_root: String,
}

impl WindowsLocations {
    /// Read the real locations from the environment, falling back to the
    /// conventional layout for any variable Windows did not set.
    pub fn from_env() -> Self {
        Self::from_lookup(&|name| std::env::var(name).ok())
    }

    /// Build from an arbitrary variable lookup.
    ///
    /// Taking the lookup as a parameter keeps the fallback chain a pure
    /// function, so tests can cover it without mutating the process
    /// environment, which is racy under the parallel test harness.
    fn from_lookup(lookup: &dyn Fn(&str) -> Option<String>) -> Self {
        let read = |name: &str| lookup(name).filter(|value| !value.is_empty());

        let system_drive = trim_trailing_separator(
            &read(SYSTEM_DRIVE).unwrap_or_else(|| DEFAULT_SYSTEM_DRIVE.to_owned()),
        );

        // A 64-bit process sees the native Program Files under either name, but
        // `ProgramW6432` is the one that stays correct under WOW64 redirection.
        let program_files = read("ProgramW6432")
            .or_else(|| read("ProgramFiles"))
            .unwrap_or_else(|| format!("{system_drive}{SEPARATOR}Program Files"));

        let program_files_x86 = read("ProgramFiles(x86)")
            .unwrap_or_else(|| format!("{system_drive}{SEPARATOR}Program Files (x86)"));

        let program_data =
            read("ProgramData").unwrap_or_else(|| format!("{system_drive}{SEPARATOR}ProgramData"));

        let system_root = read("SystemRoot")
            .or_else(|| read("windir"))
            .unwrap_or_else(|| format!("{system_drive}{SEPARATOR}Windows"));

        Self {
            system_drive,
            program_files: trim_trailing_separator(&program_files),
            program_files_x86: trim_trailing_separator(&program_files_x86),
            program_data: trim_trailing_separator(&program_data),
            system_root: trim_trailing_separator(&system_root),
        }
    }

    /// The conventional layout for a Windows install on the given drive.
    ///
    /// Used by tests so plan validation can run on any platform.
    pub fn conventional(drive: &str) -> Self {
        let system_drive = trim_trailing_separator(drive);
        Self {
            program_files: format!("{system_drive}{SEPARATOR}Program Files"),
            program_files_x86: format!("{system_drive}{SEPARATOR}Program Files (x86)"),
            program_data: format!("{system_drive}{SEPARATOR}ProgramData"),
            system_root: format!("{system_drive}{SEPARATOR}Windows"),
            system_drive,
        }
    }

    fn placeholder_value(&self, name: &str) -> Option<&str> {
        match name {
            SYSTEM_DRIVE => Some(&self.system_drive),
            PROGRAM_FILES => Some(&self.program_files),
            PROGRAM_FILES_X86 => Some(&self.program_files_x86),
            PROGRAM_DATA => Some(&self.program_data),
            SYSTEM_ROOT => Some(&self.system_root),
            _ => None,
        }
    }

    /// Expand `template` and confirm the result is safe to delete.
    pub fn resolve(&self, template: &str) -> Result<String, PathError> {
        let expanded = self.expand(template)?;
        self.validate(&expanded)?;
        Ok(expanded)
    }

    fn expand(&self, template: &str) -> Result<String, PathError> {
        let mut expanded = String::with_capacity(template.len());
        let mut rest = template;

        while let Some(open) = rest.find(PLACEHOLDER_OPEN) {
            expanded.push_str(&rest[..open]);
            let after_open = &rest[open + PLACEHOLDER_OPEN.len_utf8()..];

            let close = after_open
                .find(PLACEHOLDER_CLOSE)
                .ok_or_else(|| PathError::UnterminatedPlaceholder(template.to_owned()))?;

            let name = &after_open[..close];
            let value = self
                .placeholder_value(name)
                .ok_or_else(|| PathError::UnknownPlaceholder(name.to_owned()))?;

            expanded.push_str(value);
            rest = &after_open[close + PLACEHOLDER_CLOSE.len_utf8()..];
        }

        expanded.push_str(rest);
        Ok(expanded)
    }

    /// Reject anything we are not willing to hand to a recursive delete.
    pub fn validate(&self, path: &str) -> Result<(), PathError> {
        let segments = segments_below_drive(path)?;

        if self.is_protected(path) {
            return Err(PathError::ProtectedLocation(path.to_owned()));
        }

        if segments.len() < MINIMUM_SEGMENTS_BELOW_DRIVE {
            return Err(PathError::TooShallow(path.to_owned()));
        }

        Ok(())
    }

    fn is_protected(&self, path: &str) -> bool {
        let candidate = trim_trailing_separator(path);
        self.protected_locations()
            .iter()
            .any(|protected| protected.eq_ignore_ascii_case(&candidate))
    }

    /// Directories that exist on every Windows install and must survive.
    fn protected_locations(&self) -> Vec<String> {
        let system32 = format!("{}{SEPARATOR}System32", self.system_root);
        vec![
            self.system_drive.clone(),
            self.program_files.clone(),
            self.program_files_x86.clone(),
            self.program_data.clone(),
            self.system_root.clone(),
            format!("{}{SEPARATOR}Common Files", self.program_files),
            format!("{}{SEPARATOR}Common Files", self.program_files_x86),
            format!("{}{SEPARATOR}Users", self.system_drive),
            format!("{system32}{SEPARATOR}drivers"),
            system32,
        ]
    }
}

// ─── Path parsing ────────────────────────────────────────────────────────────

const DEFAULT_SYSTEM_DRIVE: &str = "C:";
const DRIVE_SUFFIX: char = ':';
const CURRENT_DIR_SEGMENT: &str = ".";
const PARENT_DIR_SEGMENT: &str = "..";

fn trim_trailing_separator(path: &str) -> String {
    path.trim_end_matches(SEPARATOR).to_owned()
}

/// Split `X:\a\b` into `["a", "b"]`, rejecting anything that is not a
/// well-formed absolute Windows path.
fn segments_below_drive(path: &str) -> Result<Vec<&str>, PathError> {
    let mut characters = path.chars();
    let has_drive_prefix = matches!(characters.next(), Some(letter) if letter.is_ascii_alphabetic())
        && characters.next() == Some(DRIVE_SUFFIX)
        && characters.next() == Some(SEPARATOR);

    if !has_drive_prefix {
        return Err(PathError::NotAbsolute(path.to_owned()));
    }

    let drive_prefix_length = "X:\\".len();
    let below_drive = path[drive_prefix_length..].trim_end_matches(SEPARATOR);
    if below_drive.is_empty() {
        return Ok(Vec::new());
    }

    let segments: Vec<&str> = below_drive.split(SEPARATOR).collect();

    let is_malformed = segments.iter().any(|segment| {
        segment.is_empty() || *segment == CURRENT_DIR_SEGMENT || *segment == PARENT_DIR_SEGMENT
    });

    if is_malformed {
        return Err(PathError::MalformedSegment(path.to_owned()));
    }

    Ok(segments)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn locations() -> WindowsLocations {
        WindowsLocations::conventional("C:")
    }

    // ── expansion ────────────────────────────────────────────────────────────

    #[test]
    fn expands_program_files_placeholder() {
        assert_eq!(
            locations().resolve(r"{ProgramFiles}\McAfee").unwrap(),
            r"C:\Program Files\McAfee"
        );
    }

    #[test]
    fn expands_every_supported_placeholder() {
        let locations = locations();
        let expansions = [
            (r"{SystemDrive}\Users\Public", r"C:\Users\Public"),
            (r"{ProgramFiles}\Norton", r"C:\Program Files\Norton"),
            (
                r"{ProgramFilesX86}\Norton",
                r"C:\Program Files (x86)\Norton",
            ),
            (r"{ProgramData}\Norton", r"C:\ProgramData\Norton"),
            (
                r"{SystemRoot}\System32\drivers\aswSP.sys",
                r"C:\Windows\System32\drivers\aswSP.sys",
            ),
        ];

        for (template, expected) in expansions {
            assert_eq!(locations.resolve(template).unwrap(), expected);
        }
    }

    #[test]
    fn expansion_follows_a_relocated_windows_drive() {
        assert_eq!(
            WindowsLocations::conventional("D:")
                .resolve(r"{ProgramData}\AVG")
                .unwrap(),
            r"D:\ProgramData\AVG"
        );
    }

    #[test]
    fn unknown_placeholder_is_rejected() {
        assert_eq!(
            locations().resolve(r"{NoSuchRoot}\McAfee"),
            Err(PathError::UnknownPlaceholder("NoSuchRoot".to_owned()))
        );
    }

    #[test]
    fn unterminated_placeholder_is_rejected() {
        assert!(matches!(
            locations().resolve(r"{ProgramFiles\McAfee"),
            Err(PathError::UnterminatedPlaceholder(_))
        ));
    }

    #[test]
    fn template_without_placeholders_passes_through() {
        assert_eq!(
            locations().resolve(r"C:\Program Files\McAfee").unwrap(),
            r"C:\Program Files\McAfee"
        );
    }

    // ── safety ───────────────────────────────────────────────────────────────

    #[test]
    fn drive_root_is_never_a_valid_target() {
        assert!(matches!(
            locations().resolve(r"{SystemDrive}\"),
            Err(PathError::ProtectedLocation(_))
        ));
    }

    #[test]
    fn shallow_paths_are_rejected() {
        assert!(matches!(
            locations().resolve(r"{SystemDrive}\McAfee"),
            Err(PathError::TooShallow(_))
        ));
    }

    #[test]
    fn protected_system_locations_are_rejected() {
        let locations = locations();
        let protected = [
            r"{SystemRoot}\System32",
            r"{SystemRoot}\System32\drivers",
            r"{ProgramFiles}\Common Files",
            r"{ProgramFilesX86}\Common Files",
            r"{SystemDrive}\Users",
        ];

        for template in protected {
            assert!(
                matches!(
                    locations.resolve(template),
                    Err(PathError::ProtectedLocation(_))
                ),
                "{template} must be refused"
            );
        }
    }

    #[test]
    fn protected_check_ignores_case_and_trailing_separator() {
        assert!(matches!(
            locations().resolve(r"c:\windows\system32\"),
            Err(PathError::ProtectedLocation(_))
        ));
    }

    #[test]
    fn relative_and_empty_segments_are_rejected() {
        let locations = locations();
        for path in [r"C:\Program Files\..\Windows", r"C:\Program Files\\McAfee"] {
            assert!(
                matches!(locations.resolve(path), Err(PathError::MalformedSegment(_))),
                "{path} must be refused"
            );
        }
    }

    #[test]
    fn non_absolute_paths_are_rejected() {
        let locations = locations();
        for path in [r"Program Files\McAfee", r"\McAfee", r"\\server\share\x"] {
            assert!(
                matches!(locations.resolve(path), Err(PathError::NotAbsolute(_))),
                "{path} must be refused"
            );
        }
    }

    #[test]
    fn deep_product_paths_are_accepted() {
        assert!(
            locations()
                .resolve(r"{ProgramFiles}\Common Files\McAfee")
                .is_ok()
        );
    }

    // ── environment ──────────────────────────────────────────────────────────

    #[test]
    fn from_env_produces_absolute_validated_roots() {
        let locations = WindowsLocations::from_env();
        assert!(
            locations
                .resolve(r"{ProgramFiles}\Wixen Test\Nested")
                .is_ok()
        );
        assert!(!locations.system_drive.ends_with(SEPARATOR));
        assert!(!locations.program_files.ends_with(SEPARATOR));
    }

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        move |name| {
            owned
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn environment_values_win_over_the_conventional_layout() {
        let locations = WindowsLocations::from_lookup(&lookup(&[
            ("SystemDrive", "E:"),
            ("ProgramW6432", r"E:\Apps"),
            ("ProgramFiles(x86)", r"E:\Apps32"),
            ("ProgramData", r"F:\Shared"),
            ("SystemRoot", r"E:\WinNT"),
        ]));

        assert_eq!(
            locations.resolve(r"{ProgramFiles}\McAfee").unwrap(),
            r"E:\Apps\McAfee"
        );
        assert_eq!(
            locations.resolve(r"{ProgramFilesX86}\McAfee").unwrap(),
            r"E:\Apps32\McAfee"
        );
        assert_eq!(
            locations.resolve(r"{ProgramData}\McAfee").unwrap(),
            r"F:\Shared\McAfee"
        );
        assert_eq!(
            locations
                .resolve(r"{SystemRoot}\System32\drivers\x.sys")
                .unwrap(),
            r"E:\WinNT\System32\drivers\x.sys"
        );
    }

    #[test]
    fn missing_variables_fall_back_to_the_system_drive() {
        let locations = WindowsLocations::from_lookup(&lookup(&[("SystemDrive", "D:")]));
        assert_eq!(locations, WindowsLocations::conventional("D:"));
    }

    #[test]
    fn an_empty_environment_falls_back_to_drive_c() {
        assert_eq!(
            WindowsLocations::from_lookup(&lookup(&[])),
            WindowsLocations::conventional(DEFAULT_SYSTEM_DRIVE)
        );
    }

    #[test]
    fn empty_variables_are_treated_as_unset() {
        // Windows hands back an empty string for some variables in stripped
        // service environments; using it verbatim would yield `\McAfee`.
        let locations = WindowsLocations::from_lookup(&lookup(&[
            ("SystemDrive", ""),
            ("ProgramW6432", ""),
            ("ProgramData", ""),
        ]));

        assert_eq!(
            locations,
            WindowsLocations::conventional(DEFAULT_SYSTEM_DRIVE)
        );
    }

    #[test]
    fn program_files_falls_back_to_the_wow64_redirected_name() {
        let locations =
            WindowsLocations::from_lookup(&lookup(&[("ProgramFiles", r"C:\Legacy Apps")]));
        assert_eq!(
            locations.resolve(r"{ProgramFiles}\Norton").unwrap(),
            r"C:\Legacy Apps\Norton"
        );
    }

    #[test]
    fn system_root_falls_back_to_windir() {
        let locations = WindowsLocations::from_lookup(&lookup(&[("windir", r"C:\WINNT")]));
        assert_eq!(
            locations
                .resolve(r"{SystemRoot}\System32\drivers\x.sys")
                .unwrap(),
            r"C:\WINNT\System32\drivers\x.sys"
        );
    }

    #[test]
    fn trailing_separators_in_environment_values_are_trimmed() {
        let locations = WindowsLocations::from_lookup(&lookup(&[
            ("SystemDrive", "C:\\"),
            ("ProgramData", "D:\\Data\\"),
        ]));
        assert_eq!(
            locations.resolve(r"{ProgramData}\AVG").unwrap(),
            r"D:\Data\AVG"
        );
    }

    #[test]
    fn path_error_messages_name_the_offending_input() {
        let message = PathError::ProtectedLocation(r"C:\Windows".to_owned()).to_string();
        assert!(message.contains(r"C:\Windows"));
    }
}
