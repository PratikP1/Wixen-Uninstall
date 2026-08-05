# Changelog

All notable changes to Wixen Uninstaller are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/PratikP1/Wixen-Uninstall/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/PratikP1/Wixen-Uninstall/releases/tag/v0.4.0
