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

/// Collect the version strings that follow every occurrence of `prefix`.
fn versions_after(text: &str, prefix: &str) -> Vec<String> {
    text.match_indices(prefix)
        .map(|(index, _)| {
            text[index + prefix.len()..]
                .chars()
                .take_while(|character| character.is_ascii_digit() || *character == '.')
                .collect::<String>()
                .trim_end_matches('.')
                .to_owned()
        })
        .filter(|version| !version.is_empty())
        .collect()
}

/// Every place a reader is told the name of the installer they should download.
const FILES_NAMING_THE_INSTALLER: &[&str] = &["README.md", "SECURITY.md", "docs/release-notes.md"];

#[test]
fn the_documented_installer_filename_matches_the_package_version() {
    for relative_path in FILES_NAMING_THE_INSTALLER {
        let text = repository_file(relative_path);
        let mentioned = versions_after(&text, "WixenUninstaller-Setup-");

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

    let tags = versions_after(&readme, "git tag v");
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

    for pushed in versions_after(&readme, "git push origin v") {
        assert_eq!(pushed, PACKAGE_VERSION);
    }
}

#[test]
fn the_installer_scripts_fallback_version_matches_the_package() {
    // CI always passes /DAppVersion, but a maintainer compiling the .iss by
    // hand gets this default, and it ends up in Add or Remove Programs.
    let script = repository_file("wixen_uninstall.iss");
    let versions = versions_after(&script, r#"#define AppVersion   ""#);

    assert_eq!(
        versions,
        vec![PACKAGE_VERSION.to_owned()],
        "wixen_uninstall.iss has a stale AppVersion fallback"
    );
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
