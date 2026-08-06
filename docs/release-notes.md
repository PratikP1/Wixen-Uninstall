Wixen Uninstaller removes stubborn, misbehaving Windows applications. It also
removes the companion browsers, VPNs, and tune-up tools that linger after the
official uninstaller claims to be done. It ships today with definitions for four
notoriously stubborn security suites; the removal engine is general, and the
list is meant to grow.

## Supported products

| Product | Companion leftovers also removed |
|---|---|
| McAfee Total Protection | LiveSafe, WebAdvisor, SiteAdvisor |
| Norton 360 / Norton Security | Secure VPN, Norton Utilities |
| Avast Antivirus / Premium Security | Secure Browser, Avast Cleanup |
| AVG AntiVirus / Internet Security | Secure Browser, AVG TuneUp |

## Installing

1. Download `WixenUninstaller-Setup-0.5.0.exe` below.
2. Windows SmartScreen will warn you, because the installer is not code-signed.
   Verify the download against the `.sha256` file first, then choose **More
   info → Run anyway**.
3. Run the installer and follow the wizard.
4. Launch **Wixen Uninstaller** from the Start menu. It requests Administrator,
   so accept the UAC prompt.

Verify your download:

```powershell
Get-FileHash -Algorithm SHA256 .\WixenUninstaller-Setup-0.5.0.exe
```

## Accessibility

Wixen is built on native Windows task dialogs, the same modern dialog Windows
itself uses. That means NVDA, JAWS, and Narrator read every screen without any
special configuration, and everything scales correctly on high-DPI displays.

Each product is its own labelled button, so a screen reader announces the
product name and what its cleanup sweeps up, rather than a bare "Yes" or "No".

- <kbd>Tab</kbd> / <kbd>Shift+Tab</kbd> move between controls.
- <kbd>Up</kbd> / <kbd>Down</kbd> move between the product buttons.
- <kbd>Enter</kbd> or <kbd>Space</kbd> activates the focused control.
- <kbd>Alt</kbd> plus the underlined letter activates a button directly.
- <kbd>Esc</kbd> cancels the current screen or quits.
- <kbd>F1</kbd> opens the installed HTML help guide.

## Nothing is deleted before you have seen the list

The confirmation screen has a **Show what will be removed** section listing the
exact folders, driver files, services, scheduled tasks, and registry keys.
Focus starts on **Cancel**, so pressing <kbd>Enter</kbd> by reflex never begins
a removal.

While it works, a progress bar names each stage rather than the window simply
freezing.

## Removing apps that fight back, without Safe Mode

Some applications resist removal by locking files, denying permissions, or
loading a kernel driver that blocks deletion while Windows runs (Avast and AVG
are the standout case). Wixen handles these in normal Windows, with your screen
reader and audio running. You do not need Safe Mode.

When an ordinary removal is blocked, Wixen escalates on its own:

- it runs the product's own silent uninstaller, read from the registry, which
  can switch off self-protection from the inside;
- it elevates to `NT AUTHORITY\SYSTEM` where Administrator is not enough;
- it takes ownership of permission-locked files and folders, then retries;
- it queues anything still locked for deletion on your next restart, and
  registers itself to finish automatically after you reboot normally.

If files were queued, restart Windows when it is convenient. Wixen runs once
more on its own to finish the cleanup, with your screen reader and audio working
as usual.

## Before you start

- **Restart afterwards.** Removing kernel drivers and services only fully takes
  effect after a reboot. If Wixen queued locked files, it also finishes the
  cleanup automatically after that restart.
- Wixen never deletes a driver file while its service is still registered. If
  removal is blocked, the file is reported as skipped rather than deleted,
  because deleting it could stop Windows from starting.

## Known limitation

The automated removal of self-defending products is new in this release. Its
decisions are covered by unit and mutation tests, but its effect against a live
installation has not yet been verified on a Windows machine with one of these
suites installed. If a removal leaves something behind, the report names exactly
what remains, so you can run Wixen again or file an issue.

## Requirements

- 64-bit Windows 10 or Windows 11
- Administrator rights

## Removing Wixen

Settings → Apps → Installed apps → **Wixen Uninstaller** → Uninstall.

Want another product supported?
[File an issue.](https://github.com/PratikP1/Wixen-Uninstall/issues)
