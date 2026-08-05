# Wixen-Uninstall

A user-friendly, totally accessible, bullshit-free uninstaller for stubborn
security products like McAfee, Norton, Avast, and AVG, plus the companion
browser, VPN, cleanup, and tune-up add-ons that often linger after uninstall.

**Author:** PratikP1

---

## Features

- Removes **McAfee Total Protection**, **Norton 360 / Norton Security**,
  **Avast Antivirus / Avast Premium Security**, and
  **AVG AntiVirus / AVG Internet Security** completely: files, directories,
  registry keys, Windows services, and scheduled tasks.
- Sweeps same-vendor companion products during cleanup, including **McAfee
  LiveSafe / WebAdvisor**, **Norton Secure VPN / Utilities**, **Avast Secure
  Browser / Cleanup**, and **AVG Secure Browser / TuneUp** leftovers.
- Fully accessible on Windows - native Win32 dialogs for selection,
  confirmation, and status, with keyboard-only navigation, clear
  **Tab/Shift+Tab**, **Enter/Space**, and **Esc** guidance, and screen-reader
  friendly system controls. Press **F1** in the Windows UI to open the bundled
  HTML help guide. The CLI remains available as a fallback for development and
  test environments.
- Explicitly targets **64-bit Windows 10 and Windows 11**.
- Requests Administrator on launch via an embedded application manifest, so the
  Start-menu shortcut triggers a UAC prompt instead of silently failing every
  removal action.
- Deletes self-healing scheduled tasks before services and file paths so
  stubborn suites like Avast and AVG cannot immediately reinstall themselves
  during cleanup.
- **Will not brick your boot.** A kernel driver's `.sys` file is only deleted
  once its service has been removed. If self-protection blocks the service, the
  driver file is left alone and reported as skipped, because deleting the image
  of a still-registered boot-start driver can stop Windows from starting.
- **Follows your actual Windows layout.** Locations are resolved from
  `%ProgramFiles%`, `%ProgramData%`, and `%SystemRoot%` rather than assuming
  `C:\`, and every target is validated before deletion: drive roots, Windows,
  System32, Program Files, ProgramData, and Users can never be targeted.
- Packaged as a standalone Windows installer via **Inno Setup**.
- Ships with an installed HTML help guide for release builds.
- Written in pure Rust; no runtime or build dependencies.
- Wixen itself is cleanly uninstallable through the standard Windows
  **Add or Remove Programs** flow.
- Tested with red/green TDD, mutation testing (`cargo-mutants`), and fuzz
  testing (`cargo-fuzz`).

---

## Supported products

| # | Product | Notes |
|---|---------|-------|
| 1 | McAfee Total Protection | Also removes common McAfee LiveSafe and WebAdvisor / SiteAdvisor leftovers |
| 2 | Norton 360 / Norton Security | Also removes common Norton Secure VPN and Norton Utilities leftovers |
| 3 | Avast Antivirus / Avast Premium Security | Also removes common Avast Secure Browser and Avast Cleanup leftovers; Safe Mode may be needed if self-protection blocks normal-mode cleanup |
| 4 | AVG AntiVirus / AVG Internet Security | Also removes common AVG Secure Browser and AVG TuneUp leftovers; Safe Mode may be needed if self-protection blocks normal-mode cleanup |

Want another product removed? [File an issue.](https://github.com/PratikP1/Wixen-Uninstall/issues)

## Supported Windows versions

- Windows 10, 64-bit
- Windows 11, 64-bit
- The packaged installer is currently `x86_64-pc-windows-msvc`; 32-bit Windows
  is not supported.

---

## Quick start (Windows)

1. Download the latest `WixenUninstaller-Setup-*.exe` from the
   [Releases](https://github.com/PratikP1/Wixen-Uninstall/releases) page.
2. Right-click the installer and choose **Run as administrator**.
3. Follow the wizard.  After installation, launch **Wixen Uninstaller** from
   the Start menu.
4. A native Windows dialog will appear and ask which product you want to
   remove.  Use **Tab** / **Shift+Tab** to move between buttons, **Enter** or
   **Space** to activate the focused button, **Esc** to open the next page of
   products or quit, and **F1** to open the installed HTML help guide.
5. Confirm the generated removal plan in the follow-up dialog.  Use **Tab** /
   **Shift+Tab** to move focus, **Enter** or **Space** to start, **Esc** to go
   back, and **F1** to open the installed HTML help guide.
6. If you are removing Avast or AVG and normal-mode cleanup reports access
   errors, reboot into **Windows Safe Mode** and run Wixen again.
7. Review the completion report shown at the end. It separates real errors from
   actions skipped for safety.
8. **Restart Windows** to finish the cleanup — removing kernel drivers and
   services only fully takes effect after a reboot.

> **Note:** The tool must be run with Administrator privileges.  Both the
> installer and the installed application request elevation, so Windows prompts
> you automatically.
>
> **SmartScreen:** Releases are not code-signed, so Windows will warn you the
> first time you run the installer.  Verify your download against the `.sha256`
> file published alongside it, then choose **More info > Run anyway**:
>
> ```powershell
> Get-FileHash -Algorithm SHA256 .\WixenUninstaller-Setup-0.1.0.exe
> ```
>
> **Help file:** The installer places `WixenUninstallerHelp.html` next to the
> application executable and opens it when you press **F1** from the Windows UI.
>
> **Wixen uninstall:** To remove Wixen itself, use **Settings > Apps > Installed
> apps** (or **Add or Remove Programs**) and uninstall **Wixen Uninstaller** like
> any other Windows application.

---

## Building from source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) 1.88 or newer (2024 edition; 1.88 is
  where let-chains stabilised)
- [Inno Setup 6](https://jrsoftware.org/isinfo.php) (for building the
  installer; Windows 10/11 64-bit only)

### Compile

```sh
# Debug build (for development)
cargo build

# Release build (for the installer)
cargo build --release --target x86_64-pc-windows-msvc
```

### Run tests

```sh
cargo test --features test-utils
```

The `test-utils` feature is required: it exposes `StubExecutor`, and without it
the integration test target is skipped rather than run.

### Lint

Clippy must be run against the Windows target too — the Win32 modules are
compiled out on Linux, so host-only linting cannot see them.

```sh
cargo fmt --all --check
cargo clippy --all-targets --features test-utils -- -D warnings
rustup target add x86_64-pc-windows-msvc
cargo clippy --all-targets --features test-utils \
  --target x86_64-pc-windows-msvc -- -D warnings
```

### Run mutation testing

```sh
cargo install cargo-mutants
cargo mutants
```

Configuration lives in `.cargo/mutants.toml` — that exact path, which is the
only one cargo-mutants reads. It enables the `test-utils` feature and skips the
Windows-only modules, which are compiled out on Linux and could never be
covered there.

### Run fuzz targets (requires nightly)

```sh
rustup install nightly
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run fuzz_parse_input      -- -max_total_time=60
cargo +nightly fuzz run fuzz_from_slug        -- -max_total_time=60
cargo +nightly fuzz run fuzz_from_menu_index  -- -max_total_time=60
cargo +nightly fuzz run fuzz_resolve_path     -- -max_total_time=60
```

`fuzz_resolve_path` is the important one: it hammers the parser that decides
what Wixen is willing to delete recursively.

### Build the Windows installer

1. Compile the release binary (see above).
2. Open `wixen_uninstall.iss` in the Inno Setup Compiler. The script auto-detects
   either `target\x86_64-pc-windows-msvc\release` or `target\release`, and CI
   can override `AppVersion`, `BinaryDir`, and `OutputDir` with `ISCC /D...`
   arguments.
3. Click **Build > Compile** (or press `F9`).
4. The installer is written to `installer_output/`.

### CI

`.github/workflows/ci.yml` runs on every pull request and on pushes to `main`:

- `cargo fmt --all --check`
- Clippy on the host target **and** on `x86_64-pc-windows-msvc`, both with
  `-D warnings`
- `cargo test --locked --features test-utils` on Linux and Windows
- A short smoke run of every fuzz target
- A Windows release build that asserts the elevation manifest is embedded,
  compiles `wixen_uninstall.iss`, and uploads the installer as an artifact

### Cutting a release

`.github/workflows/release.yml` is driven by tags:

```sh
# 1. Bump the version in Cargo.toml and add a CHANGELOG entry.
# 2. Tag it — the workflow refuses to publish if the tag and Cargo.toml
#    version disagree.
git tag v0.1.0
git push origin v0.1.0
```

The workflow re-runs the full check suite, builds the installer, generates a
SHA-256 checksum, and publishes a GitHub Release using `docs/release-notes.md`
as the body.

---

## Architecture

```
src/
  lib.rs        - module declarations
  product.rs    - Product enum + parsing helpers
  paths.rs      - Windows location resolution + delete-target validation
  plan.rs       - RemovalPlan (pure data; no I/O)
  executor.rs   - Executor trait + LiveExecutor + StubExecutor
  elevation.rs  - Administrator privilege detection
  menu.rs       - accessible CLI fallback menu
  ui.rs         - Win32 dialog UI + CLI fallback orchestration
  main.rs       - entry point

docs/
  WixenUninstallerHelp.html - installed HTML help guide
  release-notes.md          - body of the published GitHub Release

tests/
  integration.rs - end-to-end pipeline tests

fuzz/
  fuzz_targets/
    fuzz_parse_input.rs
    fuzz_from_slug.rs
    fuzz_from_menu_index.rs
    fuzz_resolve_path.rs

build.rs                  - embeds the Windows elevation manifest
wixen_uninstall.manifest  - requireAdministrator, longPathAware, DPI awareness
wixen_uninstall.iss       - Inno Setup packaging script
```

### How a removal is ordered, and why

`executor::execute` always runs in this order, and the order is load-bearing:

1. **Scheduled tasks**, so a self-repair task cannot reinstate what comes next.
2. **Services**, releasing file handles and — critically — deregistering
   drivers before their images are touched.
3. **Files**, with guarded driver images skipped if step 2 failed for them.
4. **Registry keys**, which is what finally makes the product invisible to
   Windows.

---

## Security

See [SECURITY.md](SECURITY.md) for how to report a vulnerability and for the
constraints Wixen places on its own privileges.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT
