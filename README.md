# Wixen-Uninstall

A user-friendly, totally accessible, bullshit-free uninstaller for unwanted
products like McAfee and Norton crapware.

**Author:** PratikP1

---

## Features

- Removes **McAfee Total Protection** and **Norton 360 / Norton Security** completely:
  files, directories, registry keys, Windows services, and scheduled tasks.
- Fully accessible on Windows — native Win32 dialogs for selection,
  confirmation, and status, with keyboard-only navigation and screen-reader
  friendly system controls.  The CLI remains available as a fallback for
  development and test environments.
- Packaged as a standalone Windows installer via **Inno Setup**.
- Written in pure Rust; no runtime dependencies.
- Tested with red/green TDD, mutation testing (`cargo-mutants`), and fuzz
  testing (`cargo-fuzz`).

---

## Supported products

| # | Product |
|---|---------|
| 1 | McAfee Total Protection |
| 2 | Norton 360 / Norton Security |

Want another product removed? [File an issue.](https://github.com/PratikP1/Wixen-Uninstall/issues)

---

## Quick start (Windows)

1. Download the latest `WixenUninstaller-Setup-*.exe` from the
   [Releases](https://github.com/PratikP1/Wixen-Uninstall/releases) page.
2. Right-click the installer and choose **Run as administrator**.
3. Follow the wizard.  After installation, launch **Wixen Uninstaller** from
   the Start menu.
4. A native Windows dialog will appear and ask which product you want to
   remove.
5. Confirm the generated removal plan in the follow-up dialog.
6. Review the completion report shown at the end.

> **Note:** The tool must be run with Administrator privileges.  The Inno Setup
> installer handles this automatically via a UAC prompt.

---

## Building from source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable, 2024 edition)
- [Inno Setup 6](https://jrsoftware.org/isinfo.php) (for building the
  installer; Windows only)

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
