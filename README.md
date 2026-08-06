# Wixen-Uninstall

A user-friendly, totally accessible, bullshit-free uninstaller for stubborn,
misbehaving Windows applications — the kind that resist a normal uninstall by
locking files, denying permissions, scattering leftovers, or actively defending
themselves. It ships today with built-in definitions for four notoriously
stubborn security suites — McAfee, Norton, Avast, and AVG — plus the companion
browser, VPN, cleanup, and tune-up add-ons that linger after them. The removal
engine is general, though: every app goes through the same escalation, and the
catalog of supported products is meant to grow.

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
- Fully accessible on Windows - built on native **task dialogs**, the same
  modern dialog Windows itself uses. Every product is its own labelled button
  rather than a "Yes"/"No" whose meaning lives in the body text, so NVDA, JAWS,
  and Narrator announce exactly what each control does. Keyboard-only
  throughout: **Tab/Shift+Tab** between controls, **arrow keys** between
  product buttons, **Enter/Space** to activate, **Alt+letter** access keys,
  **Esc** to cancel, and **F1** for the bundled help guide. Dialogs scale
  correctly on high-DPI displays. The CLI remains available as a fallback for
  development and test environments.
- **Shows exactly what it will delete** before it deletes anything: expand
  *Show what will be removed* on the confirmation screen for the full list of
  folders, driver files, services, scheduled tasks, and registry keys.
- **Reports live progress.** The removal runs on a worker thread behind a
  progress bar that names each stage, instead of freezing the window - which,
  with a screen reader, is indistinguishable from a crash.
- Explicitly targets **64-bit Windows 10 and Windows 11**.
- Requests Administrator on launch via an embedded application manifest, so the
  Start-menu shortcut triggers a UAC prompt instead of silently failing every
  removal action.
- Deletes self-healing scheduled tasks before services and file paths so a
  stubborn app cannot immediately reinstall itself during cleanup.
- **Will not brick your boot.** A kernel driver's `.sys` file is only deleted
  once its service has been removed. If the service cannot be removed, the driver
  file is left alone and reported as skipped, because deleting the image of a
  still-registered boot-start driver can stop Windows from starting.
- **Handles apps that fight back, without Safe Mode.** Some applications resist
  removal — locking their files, denying permissions, or loading a kernel driver
  that blocks deletion while Windows runs (Avast and AVG are the standout case).
  Rather than sending you to Safe Mode — where Windows 10 often loads no audio
  driver, so a screen reader cannot start — Wixen runs the app's *own* silent
  uninstaller, elevates to `NT AUTHORITY\SYSTEM` where Administrator is not
  enough, takes ownership of permission-locked leftovers, and queues anything
  still locked for deletion during the next **normal** restart, then finishes the
  job automatically after you reboot.
- **Follows your actual Windows layout.** Locations are resolved from
  `%ProgramFiles%`, `%ProgramData%`, and `%SystemRoot%` rather than assuming
  `C:\`, and every target is validated before deletion: drive roots, Windows,
  System32, Program Files, ProgramData, and Users can never be targeted.
- Packaged as a standalone Windows installer via **Inno Setup**.
- Ships with an installed HTML help guide for release builds.
- Written in pure Rust; no runtime or build dependencies.
- Wixen itself is cleanly uninstallable through the standard Windows
  **Add or Remove Programs** flow.
- Tested with red/green TDD, and checked with mutation testing
  (`cargo-mutants`) and fuzzing (`cargo-fuzz`) — both enforced in CI, so a
  behaviour that no test pins down fails the build.

---

## Supported products

| # | Product | Notes |
|---|---------|-------|
| 1 | McAfee Total Protection | Also removes common McAfee LiveSafe and WebAdvisor / SiteAdvisor leftovers |
| 2 | Norton 360 / Norton Security | Also removes common Norton Secure VPN and Norton Utilities leftovers |
| 3 | Avast Antivirus / Avast Premium Security | Also removes common Avast Secure Browser and Avast Cleanup leftovers; self-protection is handled automatically — no Safe Mode |
| 4 | AVG AntiVirus / AVG Internet Security | Also removes common AVG Secure Browser and AVG TuneUp leftovers; self-protection is handled automatically — no Safe Mode |

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
4. Choose the product to remove.  Each one is its own button, labelled with
   the product name and the companion leftovers it also sweeps up.  Move
   between them with the **arrow keys** or **Tab**, and activate with **Enter**
   or **Space**.
5. Review the plan.  Expand **Show what will be removed** to read the exact
   folders, driver files, services, scheduled tasks, and registry keys before
   anything is deleted.  Focus starts on **Cancel**, so pressing **Enter** by
   reflex never begins a removal - choose **Remove it** (or **Alt+R**)
   deliberately.
6. Watch the progress bar.  A removal cannot be interrupted part-way, because
   stopping half-finished can leave the product broken.
7. Review the completion report.  It separates real errors from actions
   skipped for safety; expand **Show details** for the full list.
8. For self-protecting products (Avast, AVG), Wixen runs the product's own
   uninstaller and, if any file is still locked, queues it for removal during
   the next restart — no Safe Mode. If files were queued, Wixen registers itself
   to finish automatically after you restart.
9. **Restart Windows** to finish the cleanup — removing kernel drivers and
   services only fully takes effect after a reboot, and any queued files are
   deleted then.

**Running it again is safe.** Wixen re-checks every target on each run, and
anything already gone is reported as already removed rather than an error. So if
an earlier run — or an older version of Wixen — left part of a product behind,
just run it again: it re-attempts the whole removal with its full escalation
(the product's own uninstaller, take-ownership, boot-time deletion, and running
as SYSTEM) and clears most of what was left. Finishing may take a **restart**,
since anything still locked is queued for boot-time deletion and Wixen resumes
on its own after a normal reboot.

> **Note:** The tool must be run with Administrator privileges.  Both the
> installer and the installed application request elevation, so Windows prompts
> you automatically.
>
> **SmartScreen:** Releases are not code-signed, so Windows will warn you the
> first time you run the installer.  Verify your download against the `.sha256`
> file published alongside it, then choose **More info > Run anyway**:
>
> ```powershell
> Get-FileHash -Algorithm SHA256 .\WixenUninstaller-Setup-0.5.0.exe
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

The suite is in three layers:

| Target | Covers |
|---|---|
| unit tests in `src/` | pure logic: plans, path resolution, screen wording, the dialog builder and its Win32 struct layout |
| `tests/integration.rs` | the executor pipeline end to end against a stub: boot-safety guards, error accumulation, progress ordering |
| `tests/cli.rs` | the real compiled binary, driven over a pipe — the only way to reach `main` and the stdio dispatch in `ui` |

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

Every behaviour is expected to be pinned by a test that fails without it, and
mutation testing is how that is checked rather than assumed. CI runs the same
script and fails the build if anything new survives.

```sh
cargo install cargo-mutants
./scripts/check-mutants.sh
```

A **surviving mutant** means the code could be changed that way and the suite
would still pass — so that behaviour is not really covered. The fix is to write
the test that fails without it.

`.cargo/mutants-baseline.txt` lists the handful of survivors that are tolerated.
Every entry must be an *equivalent* mutant, where the mutated program is
genuinely identical and no test could tell the difference, and each carries a
comment saying why. Adding to that file is a deliberate, reviewable act; the
default answer to a survivor is a new test.

Configuration lives in `.cargo/mutants.toml` — that exact path, which is the
only one cargo-mutants reads. It enables the `test-utils` feature and skips the
code that calls into the Win32 API, which is compiled out on Linux and could
never be covered there.

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
- Mutation testing, failing if any behaviour is left unpinned by a test
- A Windows release build that asserts the elevation manifest is embedded,
  compiles `wixen_uninstall.iss`, and uploads the installer as an artifact

### Cutting a release

`.github/workflows/release.yml` is driven by tags:

```sh
# 1. Bump the version in Cargo.toml and add a CHANGELOG entry.
# 2. Tag it — the workflow refuses to publish if the tag and Cargo.toml
#    version disagree.
git tag v0.5.0
git push origin v0.5.0
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
  executor.rs   - Executor trait, the guarded sweep, and the escalating
                  execute_full / finish_resume orchestration
  uninstall.rs  - parse a registry uninstall string into program + args (pure)
  vendor.rs     - run the product's own silent uninstaller (I/O boundary)
  stats_ini.rs  - enable Avast/AVG fully-silent uninstall via stats.ini (pure)
  escalation.rs - per-artifact next step: take ownership, defer, skip, or fail
  forceful.rs   - take-ownership / delayed-delete boundary + the per-file loop
  reboot.rs     - persist a suspended run and register the RunOnce resume
  resume.rs     - ResumeState: what a run must finish after a restart (pure)
  system_exec.rs- re-launch as SYSTEM (schtasks) to run the removal headless,
                  and read its report back across the process boundary
  elevation.rs  - Administrator privilege detection
  menu.rs       - accessible CLI fallback menu
  ui.rs         - screen wording (pure, unit tested) + platform dispatch
  ui/
    task_dialog.rs - safe wrapper over Win32 TaskDialogIndirect
    windows.rs     - the Windows screens, assembled from ui.rs wording
  main.rs       - entry point; the --execute (SYSTEM) and --resume branches

docs/
  automated-removal.md      - design + plan for the escalation and resume flow
  WixenUninstallerHelp.html - installed HTML help guide
  release-notes.md          - body of the published GitHub Release

scripts/
  check-mutants.sh - mutation run + baseline comparison, used by CI

tests/
  integration.rs - executor pipeline tests against a stub
  cli.rs         - drives the compiled binary over a pipe

fuzz/
  fuzz_targets/
    fuzz_parse_input.rs
    fuzz_from_slug.rs
    fuzz_from_menu_index.rs
    fuzz_resolve_path.rs
    fuzz_parse_uninstall.rs

build.rs                  - embeds the Windows elevation manifest
wixen_uninstall.manifest  - requireAdministrator, longPathAware, DPI awareness
wixen_uninstall.iss       - Inno Setup packaging script
```

### How the UI is put together

`ui.rs` decides what every screen *says*; `ui/windows.rs` decides how it is
drawn. That split is deliberate: the wording, the details pane, the report
summary, and the truncation rules are plain functions over plain data, so they
are unit tested on Linux, while only the `TaskDialogIndirect` calls are
Windows-only.

The dialogs need version 6 of the common controls, which
`wixen_uninstall.manifest` declares. That dependency is load-bearing, not
cosmetic: `TaskDialogIndirect` does not exist in the version 5 comctl32 that
lives in System32, so a missing dependency stops the process from starting at
all. CI asserts the manifest is embedded and then launches the built binary to
prove it resolves at run time.

### How a removal is ordered, and why

`executor::execute` always runs in this order, and the order is load-bearing:

1. **Scheduled tasks**, so a self-repair task cannot reinstate what comes next.
2. **Services**, releasing file handles and — critically — deregistering
   drivers before their images are touched.
3. **Files**, with guarded driver images skipped if step 2 failed for them.
4. **Registry keys**, which is what finally makes the product invisible to
   Windows.

### How apps that resist removal are handled

`executor::execute_full` wraps that sweep in an escalation ladder so removal
never needs Safe Mode — where Windows 10 often has no audio and a screen reader
cannot start. The whole run prefers to execute as `NT AUTHORITY\SYSTEM` (via a
transient scheduled task), reaching artifacts an Administrator cannot touch; if
that relaunch cannot be arranged it falls back to running in-process under
Administrator. First it runs the app's **own silent uninstaller** (read from the
registry, never guessed), which can undo whatever the app did to defend itself.
Then the guarded sweep above runs. Each file the sweep cannot delete is
escalated one rung at a time by `forceful::resolve_file`, whose every branch is
chosen by the pure `escalation::next_step`:

- an *access-denied* file → **take ownership**, reset its ACL, retry;
- a still-*locked* file → **queue it for deletion during the next boot**
  (`MoveFileEx` with `MOVEFILE_DELAY_UNTIL_REBOOT`);
- a **guarded driver** whose service survived → **skipped at every rung**, so
  the boot-safety invariant holds on the delayed path exactly as on the
  immediate one.

When anything is queued for boot-time deletion the run is *suspended*, not
finished: `reboot::arrange_resume` writes a small state file and a `RunOnce`
entry, so after a **normal** restart Wixen relaunches with `--resume`, deletes
the now-unlocked files' registry keys, reports, and clears up. Every decision in
this ladder is pure and tested on Linux against stubs; only the Win32 calls it
drives are Windows-gated.

---

## Security

See [SECURITY.md](SECURITY.md) for how to report a vulnerability and for the
constraints Wixen places on its own privileges.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT
