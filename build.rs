//! Build script — embeds the Windows application manifest.
//!
//! Author: PratikP1
//!
//! The manifest is what makes Windows show a UAC prompt when Wixen is launched
//! from its Start-menu shortcut.  Inno Setup's `PrivilegesRequired=admin` only
//! elevates the *installer*; the installed executable needs its own manifest or
//! it runs unelevated and every removal action fails.
//!
//! Embedding is done through the MSVC linker so the crate keeps its promise of
//! no build or runtime dependencies.

const MANIFEST_FILE_NAME: &str = "wixen_uninstall.manifest";

fn main() {
    println!("cargo:rerun-if-changed={MANIFEST_FILE_NAME}");

    if !is_msvc_windows_target() {
        return;
    }

    let Ok(crate_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        println!("cargo:warning=CARGO_MANIFEST_DIR is unset; skipping manifest embedding");
        return;
    };

    let manifest = std::path::Path::new(&crate_dir).join(MANIFEST_FILE_NAME);
    if !manifest.is_file() {
        panic!(
            "{} is missing; the installed binary would run unelevated",
            manifest.display()
        );
    }

    // `/MANIFESTUAC:NO` stops the linker generating its own trustInfo block,
    // which would collide with the requestedExecutionLevel in our manifest.
    //
    // The path is quoted because a checkout under, say, `C:\Users\Jane Doe\`
    // reaches the linker through a response file when the command line grows
    // long, and there the spaces would split the option in two.
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:\"{}\"",
        manifest.display()
    );
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
}

fn is_msvc_windows_target() -> bool {
    std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}
