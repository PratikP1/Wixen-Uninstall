//! The version in `Cargo.toml` is quoted in the docs and in the installer
//! script, and those copies drift silently.
//!
//! Author: PratikP1
//!
//! A stale `WixenUninstaller-Setup-0.1.0.exe` in the README sends people
//! looking for a file the release never produced, and a stale `AppVersion` in
//! the Inno script builds an installer whose Add/Remove Programs entry
//! contradicts the release it shipped in. The release workflow already refuses
//! to publish when the git tag disagrees with `Cargo.toml`; these tests cover
//! everywhere else the number is written down.

use std::{fs, path::PathBuf};

/// The version this build was compiled with — the single source of truth.
const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

fn repository_file(relative_path: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), relative_path].iter().collect();
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("reading {relative_path}: {error}"))
}

/// Every version sitting between a fixed `prefix` and `suffix`.
///
/// Delimiting on both sides rather than scanning for digits and dots keeps
/// pre-release and build metadata intact: `0.5.0-rc.1` reaches the comparison
/// whole, instead of being truncated to `0.5.0` and failing against a
/// `CARGO_PKG_VERSION` that is correct.
fn versions_between(text: &str, prefix: &str, suffix: &str) -> Vec<String> {
    text.match_indices(prefix)
        .filter_map(|(index, _)| {
            let rest = &text[index + prefix.len()..];
            let end = rest.find(suffix)?;
            Some(rest[..end].to_owned())
        })
        // A version always starts with a digit. This skips the glob in
        // `WixenUninstaller-Setup-*.exe`, which names the artifact pattern
        // rather than any particular release.
        .filter(|version| version.starts_with(|first: char| first.is_ascii_digit()))
        .collect()
}

/// Every whitespace-delimited token following `prefix`.
///
/// Used for the command examples, where the version runs to the end of the
/// line and so may carry any SemVer suffix.
fn tokens_after(text: &str, prefix: &str) -> Vec<String> {
    text.match_indices(prefix)
        .map(|(index, _)| {
            text[index + prefix.len()..]
                .chars()
                .take_while(|character| !character.is_whitespace())
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

/// The quoted value of an Inno Setup `#define`.
///
/// Parsed rather than string-matched so that reindenting the script cannot
/// fail the test while the version itself is perfectly correct.
fn inno_define(script: &str, name: &str) -> Option<String> {
    script.lines().find_map(|line| {
        let after_directive = line.trim_start().strip_prefix("#define")?;
        let after_name = after_directive.trim_start().strip_prefix(name)?;

        // Without this, `AppVersion` would also match `AppVersionExtra`.
        if !after_name.starts_with(char::is_whitespace) {
            return None;
        }

        let quoted = after_name.trim_start().strip_prefix('"')?;
        let closing_quote = quoted.find('"')?;
        Some(quoted[..closing_quote].to_owned())
    })
}

/// Every place a reader is told the name of the installer they should download.
const FILES_NAMING_THE_INSTALLER: &[&str] = &["README.md", "SECURITY.md", "docs/release-notes.md"];

#[test]
fn the_documented_installer_filename_matches_the_package_version() {
    for relative_path in FILES_NAMING_THE_INSTALLER {
        let text = repository_file(relative_path);
        let mentioned = versions_between(&text, "WixenUninstaller-Setup-", ".exe");

        assert!(
            !mentioned.is_empty(),
            "{relative_path} should name the installer so people know what to download"
        );
        for version in mentioned {
            assert_eq!(
                version, PACKAGE_VERSION,
                "{relative_path} points at an installer this build does not produce"
            );
        }
    }
}

#[test]
fn the_documented_release_tag_matches_the_package_version() {
    let readme = repository_file("README.md");

    let tags = tokens_after(&readme, "git tag v");
    assert!(
        !tags.is_empty(),
        "the README should show how to cut a release"
    );
    for tag in tags {
        assert_eq!(
            tag, PACKAGE_VERSION,
            "the README's release instructions would be rejected by the release workflow, \
             which requires the tag to match Cargo.toml"
        );
    }

    for pushed in tokens_after(&readme, "git push origin v") {
        assert_eq!(
            pushed, PACKAGE_VERSION,
            "the README tells the reader to push a tag the release workflow \
             would reject"
        );
    }
}

#[test]
fn the_installer_scripts_fallback_version_matches_the_package() {
    // CI always passes /DAppVersion, but a maintainer compiling the .iss by
    // hand gets this default, and it ends up in Add or Remove Programs.
    let script = repository_file("wixen_uninstall.iss");
    let fallback = inno_define(&script, "AppVersion")
        .expect("wixen_uninstall.iss should define an AppVersion fallback");

    assert_eq!(
        fallback, PACKAGE_VERSION,
        "wixen_uninstall.iss has a stale AppVersion fallback"
    );
}

// ─── The helpers above, which are parsing code of their own ──────────────────

#[test]
fn a_prerelease_version_survives_extraction_intact() {
    assert_eq!(
        versions_between(
            "download WixenUninstaller-Setup-0.5.0-rc.1.exe now",
            "WixenUninstaller-Setup-",
            ".exe"
        ),
        vec!["0.5.0-rc.1".to_owned()],
        "truncating the suffix would fail a release whose docs are correct"
    );
    assert_eq!(
        tokens_after("git tag v1.0.0-beta.2+build.7\n", "git tag v"),
        vec!["1.0.0-beta.2+build.7".to_owned()]
    );
}

#[test]
fn extraction_finds_every_occurrence_and_ignores_unmatched_text() {
    let text = "Setup-1.2.3.exe and Setup-1.2.3.exe, but Setup-9.9.9.zip is not one";
    assert_eq!(
        versions_between(text, "Setup-", ".exe"),
        vec!["1.2.3".to_owned(), "1.2.3".to_owned()]
    );
    assert!(versions_between("nothing here", "Setup-", ".exe").is_empty());
    assert!(
        versions_between("Setup-*.exe", "Setup-", ".exe").is_empty(),
        "a glob names the artifact pattern, not a release"
    );
    assert!(tokens_after("git tag v\n", "git tag v").is_empty());
}

#[test]
fn the_inno_parser_tolerates_any_spacing() {
    for line in [
        r#"#define AppVersion "2.0.0""#,
        "#define\tAppVersion\t\t\"2.0.0\"",
        r#"  #define   AppVersion     "2.0.0"  ; trailing comment"#,
    ] {
        assert_eq!(
            inno_define(line, "AppVersion").as_deref(),
            Some("2.0.0"),
            "reformatting should not fail the check: {line:?}"
        );
    }
}

#[test]
fn the_inno_parser_does_not_confuse_similarly_named_defines() {
    let script = "#define AppVersionExtra \"nope\"\n#define AppVersion \"3.1.4\"\n";
    assert_eq!(inno_define(script, "AppVersion").as_deref(), Some("3.1.4"));
    assert_eq!(inno_define("#define AppName \"Wixen\"", "AppVersion"), None);
}

#[test]
fn the_changelog_documents_this_version() {
    let changelog = repository_file("CHANGELOG.md");

    assert!(
        changelog.contains(&format!("## [{PACKAGE_VERSION}]")),
        "CHANGELOG.md has no section for {PACKAGE_VERSION}; a release with no \
         entry tells users nothing about what changed"
    );
    assert!(
        changelog.contains(&format!("[{PACKAGE_VERSION}]: https://")),
        "the CHANGELOG's {PACKAGE_VERSION} link reference is missing, so the \
         heading renders as plain text"
    );
}
