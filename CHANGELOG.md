# Changelog

All notable changes to Wixen Uninstaller are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-05

First public release.

### Added

- Complete removal of **McAfee Total Protection**, **Norton 360 / Norton
  Security**, **Avast Antivirus / Avast Premium Security**, and **AVG AntiVirus
  / AVG Internet Security**: files, directories, registry keys, Windows
  services, and scheduled tasks.
- Same-vendor companion cleanup for McAfee LiveSafe / WebAdvisor, Norton Secure
  VPN / Utilities, Avast Secure Browser / Cleanup, and AVG Secure Browser /
  TuneUp leftovers.
- Native Win32 dialogs with full keyboard navigation and screen-reader support;
  <kbd>F1</kbd> opens the bundled HTML help guide.
- Accessible CLI menu as a development and testing fallback.
- Windows installer built with Inno Setup, uninstallable through Add or Remove
  Programs.
- Elevation manifest so the app requests Administrator on launch, plus
  `longPathAware` and per-monitor DPI awareness.
- Driver-image guards: a kernel driver's `.sys` file is only deleted once its
  service has been removed.
- Machine-specific path resolution through `{ProgramFiles}`-style placeholders,
  so a Windows install that is not on `C:` is cleaned correctly.
- Delete-target validation that refuses drive roots and system directories.
- Removal report distinguishing errors from actions skipped for safety, and
  advising a restart.
- Fuzz targets for the menu parser, product lookup, and path resolver.
- CI: formatting, Clippy on both the host and Windows targets, tests on Linux
  and Windows, fuzz smoke runs, and installer packaging with an assertion that
  the elevation manifest is embedded.
- Tag-triggered release workflow publishing the installer with a SHA-256
  checksum.

[Unreleased]: https://github.com/PratikP1/Wixen-Uninstall/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/PratikP1/Wixen-Uninstall/releases/tag/v0.1.0
