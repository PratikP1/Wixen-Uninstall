# Changelog

All notable changes to Wixen Uninstaller are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-08-06

### Added

- **Automated removal of stubborn, self-defending applications, without Safe
  Mode.** Some apps resist removal — locking files, denying permissions, or
  loading a kernel driver that blocks deletion while Windows runs (Avast and AVG
  are the standout case). The old advice for the hardest of them was to reboot
  into Safe Mode — but Windows 10 often loads no audio driver there, so a
  screen-reader user cannot start speech and is stranded. Removal now escalates
  automatically in normal mode instead:
  - runs the app's own silent uninstaller, read from the registry and never
    guessed, which can undo whatever it did to defend itself;
  - takes ownership and resets the ACL of permission-denied leftovers, then
    retries the deletion;
  - queues anything still locked for deletion during the next boot (`MoveFileEx`
    with `MOVEFILE_DELAY_UNTIL_REBOOT`);
  - when files are deferred, registers a `RunOnce` resume so Wixen finishes the
    job automatically after a **normal** restart, then clears its own state.
- The driver-image guard now protects the delayed-deletion path as well: a
  driver whose service is still registered is never queued for boot-time
  removal, so the boot-safety invariant holds on every rung of the ladder.
- **Removal now runs as `NT AUTHORITY\SYSTEM`.** Before running, Wixen
  re-launches itself as SYSTEM through a transient scheduled task
  (`schtasks /RU SYSTEM /RL HIGHEST`, run once and deleted) so it can pass
  artifacts ACL'd against Administrators and let vendor uninstallers run truly
  silently. A SYSTEM process has no desktop (session 0), so the interactive
  Administrator process keeps all UI — showing a "working" dialog and reading
  the SYSTEM run's report back from `%ProgramData%\Wixen\` — while the SYSTEM
  process runs the removal headless via an `--execute` entry point. SYSTEM is an
  amplifier, never a precondition: if the relaunch cannot be arranged, the
  removal runs in-process under Administrator with the live progress bar, exactly
  as before.
- `fuzz_parse_uninstall` target for the new uninstall-string parser.

### Changed

- Report, help, and product-note wording no longer mention Safe Mode; they point
  to a normal restart and, when files were deferred, an automatic resume.
- Vendor uninstall commands with a bare program name (the `msiexec.exe` an MSI
  string normalizes to) are resolved to `%SystemRoot%\System32` before launch,
  so an elevated run cannot be hijacked by a same-named binary earlier in the
  search path.
- Documentation now describes Wixen's general purpose — removing any stubborn,
  misbehaving Windows application — rather than framing it as an
  antivirus-specific tool. The shipped catalog (four security suites) is
  unchanged; the removal engine applies to every supported product alike.

### Notes

- The escalation's Windows-only I/O (the SYSTEM relaunch via `schtasks`, invoking
  the vendor uninstaller, take-ownership/`icacls`, `MoveFileEx`, and the
  `RunOnce` write) compiles and is linted on `x86_64-pc-windows-msvc`, but **CI
  cannot prove it works** against a real installed application. Every decision it drives — the
  command-line contract, the results serialization, the escalation choices — is
  unit- and mutation-tested on Linux against stubs; the effects are unverified
  until run on a Windows machine with the product installed. In particular the
  session-0 SYSTEM relaunch and the cross-process report hand-off have not been
  exercised on hardware. This must not be described as verified, or ship in a
  tagged release, before that test.

## [0.4.0] - 2026-08-05

First public release.

### Added

- Complete removal of **McAfee Total Protection**, **Norton 360 / Norton
  Security**, **Avast Antivirus / Avast Premium Security**, and **AVG AntiVirus
  / AVG Internet Security**: files, directories, registry keys, Windows
  services, and scheduled tasks.
- Same-vendor companion cleanup for McAfee LiveSafe / WebAdvisor, Norton Secure
  VPN / Utilities, Avast Secure Browser / Cleanup, and AVG Secure Browser /
  TuneUp leftovers.
- Native Win32 **task dialogs** throughout: each product is its own labelled
  command-link button rather than a "Yes"/"No" whose meaning lives in the body
  text, so screen readers announce exactly what each control does. Full
  keyboard navigation with arrow keys, <kbd>Alt</kbd> access keys, and
  <kbd>Esc</kbd>; <kbd>F1</kbd> opens the bundled HTML help guide.
- A confirmation screen that lists the exact folders, driver files, services,
  scheduled tasks, and registry keys behind "Show what will be removed", with
  focus starting on Cancel so <kbd>Enter</kbd> never begins a removal by
  reflex.
- A progress screen: the removal runs on a worker thread behind a progress bar
  that names each stage, rather than freezing the window.
- A result screen that separates real failures from actions skipped for safety,
  with the full list behind "Show details".
- Accessible CLI menu as a development and testing fallback.
- Windows installer built with Inno Setup, uninstallable through Add or Remove
  Programs.
- Application manifest declaring Administrator elevation, common controls
  version 6 (which the task dialogs require), `longPathAware`, and per-monitor
  DPI awareness.
- Driver-image guards: a kernel driver's `.sys` file is only deleted once its
  service has been removed.
- Machine-specific path resolution through `{ProgramFiles}`-style placeholders,
  so a Windows install that is not on `C:` is cleaned correctly.
- Delete-target validation that refuses drive roots and system directories.
- Removal report distinguishing errors from actions skipped for safety, and
  advising a restart.
- Fuzz targets for the menu parser, product lookup, and path resolver.
- End-to-end tests that drive the compiled binary over a pipe, covering `main`
  and the stdio dispatch that in-process tests cannot reach.
- Mutation testing enforced in CI against a reviewed baseline, so a behaviour
  that no test pins down fails the build rather than passing quietly.
- CI: formatting, Clippy on both the host and Windows targets, tests on Linux
  and Windows, fuzz smoke runs, and installer packaging. The Windows job
  asserts the manifest is embedded and then launches the built binary to prove
  the side-by-side dependency resolves at run time.
- Tag-triggered release workflow publishing the installer with a SHA-256
  checksum.

[Unreleased]: https://github.com/PratikP1/Wixen-Uninstall/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/PratikP1/Wixen-Uninstall/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/PratikP1/Wixen-Uninstall/releases/tag/v0.4.0
