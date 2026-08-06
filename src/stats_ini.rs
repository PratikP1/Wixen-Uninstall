//! Editing Avast/AVG `stats.ini` to permit a silent uninstall.
//!
//! Author: PratikP1
//!
//! `Instup.exe /instop:uninstall /silent` only runs unattended when the
//! `Common` section of `…\setup\stats.ini` contains `SilentUninstallEnabled=1`.
//! Without it the uninstaller stops on a dialog: the accessibility trap this
//! feature exists to remove.  The edit is a pure text transform, tested here;
//! the Windows layer only reads and writes the file.

use std::fmt::Write as _;

const COMMON_SECTION: &str = "[Common]";
const SILENT_KEY: &str = "SilentUninstallEnabled";
const SILENT_LINE: &str = "SilentUninstallEnabled=1";

/// Return `contents` with `SilentUninstallEnabled=1` set in `[Common]`.
///
/// An existing key (whatever its value or spacing) is replaced; a missing key
/// is inserted under an existing `[Common]`; a missing section is appended.
/// Every other line is preserved exactly, so an unrelated setting is never
/// disturbed.
pub fn enable_silent_uninstall(contents: &str) -> String {
    match find_common_section(contents) {
        Some(section) => rewrite_within_common(contents, section),
        None => append_common_section(contents),
    }
}

/// Where the `[Common]` section starts and ends (line indices, end exclusive).
struct CommonSection {
    header_line: usize,
    end_line: usize,
}

fn find_common_section(contents: &str) -> Option<CommonSection> {
    let lines: Vec<&str> = contents.lines().collect();
    let header_line = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case(COMMON_SECTION))?;

    // The section runs until the next `[...]` header or end of file.
    let end_line = lines[header_line + 1..]
        .iter()
        .position(|line| is_section_header(line))
        .map_or(lines.len(), |offset| header_line + 1 + offset);

    Some(CommonSection {
        header_line,
        end_line,
    })
}

fn rewrite_within_common(contents: &str, section: CommonSection) -> String {
    let lines: Vec<&str> = contents.lines().collect();
    let mut output = String::with_capacity(contents.len() + SILENT_LINE.len());
    let mut replaced = false;

    for (index, line) in lines.iter().enumerate() {
        let inside_common = index > section.header_line && index < section.end_line;
        if inside_common && is_key(line, SILENT_KEY) {
            let _ = writeln!(output, "{SILENT_LINE}");
            replaced = true;
        } else {
            let _ = writeln!(output, "{line}");
            // Insert right after the header when the key was absent.
            if index == section.header_line && !section_contains_key(&lines, &section) {
                let _ = writeln!(output, "{SILENT_LINE}");
            }
        }
    }

    let _ = replaced;
    output
}

fn append_common_section(contents: &str) -> String {
    let mut output = String::from(contents);
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    let _ = writeln!(output, "{COMMON_SECTION}");
    let _ = writeln!(output, "{SILENT_LINE}");
    output
}

fn section_contains_key(lines: &[&str], section: &CommonSection) -> bool {
    lines[section.header_line + 1..section.end_line]
        .iter()
        .any(|line| is_key(line, SILENT_KEY))
}

fn is_section_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[') && trimmed.ends_with(']')
}

/// `true` when `line` assigns `key` (case-insensitive, ignoring surrounding
/// space): `Key=value`, `  key = value  `, and so on.
fn is_key(line: &str, key: &str) -> bool {
    line.split_once('=')
        .is_some_and(|(candidate, _)| candidate.trim().eq_ignore_ascii_case(key))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse an INI into (section, key, value) triples so assertions do not
    /// depend on exact spacing or line order.
    fn setting(ini: &str, section: &str, key: &str) -> Option<String> {
        let mut current = String::new();
        for line in ini.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current = trimmed.to_owned();
            } else if let Some((candidate, value)) = trimmed.split_once('=')
                && candidate.trim().eq_ignore_ascii_case(key)
                && current.eq_ignore_ascii_case(section)
            {
                return Some(value.trim().to_owned());
            }
        }
        None
    }

    fn silent_value(ini: &str) -> Option<String> {
        setting(ini, "[Common]", "SilentUninstallEnabled")
    }

    // ── the setting always ends up enabled ───────────────────────────────────

    #[test]
    fn an_empty_file_gains_the_section_and_key() {
        let result = enable_silent_uninstall("");
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
    }

    #[test]
    fn a_file_without_a_common_section_gains_one() {
        let original = "[GUI]\nLanguage=en\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        // The unrelated section survives untouched.
        assert_eq!(setting(&result, "[GUI]", "Language").as_deref(), Some("en"));
    }

    #[test]
    fn a_common_section_without_the_key_gains_it() {
        let original = "[Common]\nInstalledProduct=Avast\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        assert_eq!(
            setting(&result, "[Common]", "InstalledProduct").as_deref(),
            Some("Avast"),
            "the sibling key must remain"
        );
    }

    #[test]
    fn an_existing_key_set_to_zero_is_flipped_to_one() {
        let original = "[Common]\nSilentUninstallEnabled=0\n";
        assert_eq!(
            silent_value(&enable_silent_uninstall(original)).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn an_existing_key_already_enabled_stays_enabled() {
        let original = "[Common]\nSilentUninstallEnabled=1\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        // It was replaced, not duplicated.
        assert_eq!(
            result.matches("SilentUninstallEnabled").count(),
            1,
            "the key must appear exactly once:\n{result}"
        );
    }

    // ── the edit is surgical ─────────────────────────────────────────────────

    #[test]
    fn the_key_lands_inside_common_not_a_later_section() {
        let original = "[Common]\nInstalledProduct=Avast\n\n[GUI]\nLanguage=en\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        // Must NOT have been added to the GUI section.
        assert!(setting(&result, "[GUI]", "SilentUninstallEnabled").is_none());
    }

    #[test]
    fn a_key_of_the_same_name_in_another_section_is_left_alone() {
        // Only Common's copy should be forced to 1.
        let original = "[Other]\nSilentUninstallEnabled=0\n\n[Common]\nSilentUninstallEnabled=0\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(
            setting(&result, "[Other]", "SilentUninstallEnabled").as_deref(),
            Some("0"),
            "the unrelated section must not be rewritten"
        );
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
    }

    #[test]
    fn unrelated_keys_in_common_are_preserved() {
        let original = "[Common]\nA=1\nB=2\nSilentUninstallEnabled=0\nC=3\n";
        let result = enable_silent_uninstall(original);
        for (key, value) in [("A", "1"), ("B", "2"), ("C", "3")] {
            assert_eq!(
                setting(&result, "[Common]", key).as_deref(),
                Some(value),
                "key {key} was disturbed:\n{result}"
            );
        }
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
    }

    #[test]
    fn matching_the_key_ignores_case_and_spacing() {
        let original = "[Common]\n  silentuninstallenabled = 0  \n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        assert_eq!(result.matches(SILENT_KEY).count(), 1);
    }

    #[test]
    fn the_common_header_is_matched_case_insensitively() {
        let original = "[COMMON]\nInstalledProduct=Avast\n";
        let result = enable_silent_uninstall(original);
        // The existing header is reused, not duplicated with different casing.
        assert_eq!(
            result.to_ascii_lowercase().matches("[common]").count(),
            1,
            "a second Common section would split the settings:\n{result}"
        );
        assert!(silent_value(&result).is_some());
    }

    #[test]
    fn a_section_header_is_never_a_key_line() {
        // This is why the section-boundary comparisons in `rewrite_within_common`
        // and `section_contains_key` are robust to off-by-one: the lines at those
        // boundaries are always `[...]` headers, which carry no `=`, so whether
        // a boundary is inclusive or exclusive cannot change which key is
        // rewritten. Mutants that widen those bounds are therefore equivalent,
        // and this test records the reason.
        assert!(is_section_header("[Common]"));
        assert!(!is_key("[Common]", SILENT_KEY));
        assert!(!is_key("[Anything]", "Anything"));
    }

    #[test]
    fn editing_is_idempotent() {
        let once = enable_silent_uninstall("[Common]\nInstalledProduct=Avast\n");
        let twice = enable_silent_uninstall(&once);
        assert_eq!(once, twice, "a second edit should change nothing");
    }

    // ── appending is well-formed ─────────────────────────────────────────────

    #[test]
    fn a_file_with_no_trailing_newline_does_not_glue_the_section_on() {
        // Without the separating newline, `[Common]` would be appended to the
        // last content line and stats.ini would be malformed.
        let result = enable_silent_uninstall("[GUI]\nLanguage=en");
        assert!(
            result.contains("Language=en\n[Common]") || result.contains("Language=en\r\n[Common]"),
            "the new section must start on its own line:\n{result:?}"
        );
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
    }

    #[test]
    fn an_empty_file_gains_no_leading_blank_line() {
        let result = enable_silent_uninstall("");
        assert!(
            !result.starts_with('\n'),
            "a leading blank line is malformed: {result:?}"
        );
        assert!(result.starts_with(COMMON_SECTION));
    }

    // ── section boundaries when Common is not first ──────────────────────────

    #[test]
    fn the_key_is_replaced_when_common_is_not_the_first_section() {
        // Exercises the header-offset arithmetic with a non-zero header line.
        let original = "[GUI]\nLanguage=en\n\n[Common]\nSilentUninstallEnabled=0\nOther=x\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        assert_eq!(result.matches(SILENT_KEY).count(), 1);
        assert_eq!(setting(&result, "[Common]", "Other").as_deref(), Some("x"));
    }

    #[test]
    fn the_key_is_replaced_when_it_is_the_last_line_before_another_section() {
        // Pins the section-end index: the key sits immediately before the next
        // header, so an off-by-one end boundary would treat it as outside
        // Common and insert a duplicate instead of flipping it.
        let original = "[Common]\nSilentUninstallEnabled=0\n[Later]\nX=1\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        assert_eq!(
            result.matches(SILENT_KEY).count(),
            1,
            "the last-line key must be flipped in place, not duplicated:\n{result}"
        );
    }

    #[test]
    fn a_same_named_key_in_a_section_after_common_is_left_alone() {
        // Pins the end-of-section boundary: the sweep must stop at the next
        // header, not run to end of file.
        let original = "[Common]\nInstalledProduct=Avast\n\n[Later]\nSilentUninstallEnabled=0\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(
            setting(&result, "[Later]", "SilentUninstallEnabled").as_deref(),
            Some("0"),
            "the later section must not be rewritten:\n{result}"
        );
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
    }

    #[test]
    fn a_value_ending_in_a_bracket_does_not_end_the_section_early() {
        // A line like `Hint=see [x]` ends with `]` but is not a section header;
        // treating it as one would push our key outside Common.
        let original = "[Common]\nHint=see [x]\nSilentUninstallEnabled=0\n";
        let result = enable_silent_uninstall(original);
        assert_eq!(silent_value(&result).as_deref(), Some("1"));
        assert_eq!(
            result.matches(SILENT_KEY).count(),
            1,
            "the existing key must be flipped, not joined by a second:\n{result}"
        );
        assert_eq!(
            setting(&result, "[Common]", "Hint").as_deref(),
            Some("see [x]")
        );
    }
}
