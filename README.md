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
- Fully accessible on Windows — native Win32 dialogs for selection,
  confirmation, and status, with keyboard-only navigation, clear
  **Tab/Shift+Tab**, **Enter/Space**, and **Esc** guidance, and screen-reader
  friendly system controls.  The CLI remains available as a fallback for
  development and test environments.
- Explicitly targets **64-bit Windows 10 and Windows 11**.
- Deletes self-healing scheduled tasks before services and file paths so
  stubborn suites like Avast and AVG cannot immediately reinstall themselves
  during cleanup.
- Packaged as a standalone Windows installer via **Inno Setup**.
- Written in pure Rust; no runtime dependencies.
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
   **Space** to activate the focused button, and **Esc** to open the next page
   of products or quit.
5. Confirm the generated removal plan in the follow-up dialog.  Use **Tab** /
   **Shift+Tab** to move focus, **Enter** or **Space** to start, and **Esc** to
   go back.
6. If you are removing Avast or AVG and normal-mode cleanup reports access
   errors, reboot into **Windows Safe Mode** and run Wixen again.
7. Review the completion report shown at the end.

> **Note:** The tool must be run with Administrator privileges.  The Inno Setup
> installer handles this automatically via a UAC prompt.
>
> **Wixen uninstall:** To remove Wixen itself, use **Settings ▸ Apps ▸ Installed
> apps** (or **Add or Remove Programs**) and uninstall **Wixen Uninstaller** like
> any other Windows application.

---

## Building from source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable, 2024 edition)
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

### Run mutation testing

```sh
cargo install cargo-mutants
cargo mutants --features test-utils
```

### Run fuzz targets (requires nightly)

```sh
rustup install nightly
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run fuzz_parse_input    -- -max_total_time=60
cargo +nightly fuzz run fuzz_from_slug      -- -max_total_time=60
cargo +nightly fuzz run fuzz_from_menu_index -- -max_total_time=60
```

### Build the Windows installer

1. Compile the release binary (see above).
2. Open `wixen_uninstall.iss` in the Inno Setup Compiler.
3. Click **Build ▸ Compile** (or press `F9`).
4. The installer is written to `installer_output/`.

---

## Architecture

```
src/
  lib.rs        — module declarations
  product.rs    — Product enum + parsing helpers
  plan.rs       — RemovalPlan (pure data; no I/O)
  executor.rs   — Executor trait + LiveExecutor + StubExecutor
  menu.rs       — accessible CLI fallback menu
  ui.rs         — Win32 dialog UI + CLI fallback orchestration
  main.rs       — entry point

tests/
  integration.rs — end-to-end pipeline tests

fuzz/
  fuzz_targets/
    fuzz_parse_input.rs
    fuzz_from_slug.rs
    fuzz_from_menu_index.rs

wixen_uninstall.iss  — Inno Setup packaging script
```

---

## License

MIT
