Wixen Uninstaller removes stubborn consumer security suites — and the companion
browsers, VPNs, and tune-up tools that linger after the official uninstaller
claims to be done.

## Supported products

| Product | Companion leftovers also removed |
|---|---|
| McAfee Total Protection | LiveSafe, WebAdvisor, SiteAdvisor |
| Norton 360 / Norton Security | Secure VPN, Norton Utilities |
| Avast Antivirus / Premium Security | Secure Browser, Avast Cleanup |
| AVG AntiVirus / Internet Security | Secure Browser, AVG TuneUp |

## Installing

1. Download `WixenUninstaller-Setup-0.1.0.exe` below.
2. Windows SmartScreen will warn you, because the installer is not code-signed.
   Verify the download against the `.sha256` file first, then choose **More
   info → Run anyway**.
3. Run the installer and follow the wizard.
4. Launch **Wixen Uninstaller** from the Start menu. It requests Administrator,
   so accept the UAC prompt.

Verify your download:

```powershell
Get-FileHash -Algorithm SHA256 .\WixenUninstaller-Setup-0.1.0.exe
```

## Accessibility

Wixen is built to be driven entirely from the keyboard with a screen reader:

- <kbd>Tab</kbd> / <kbd>Shift+Tab</kbd> move between buttons.
- <kbd>Enter</kbd> or <kbd>Space</kbd> activates the focused button.
- <kbd>Esc</kbd> moves to the next page of products, goes back, or quits.
- <kbd>F1</kbd> opens the installed HTML help guide.

Dialogs are standard Win32 message boxes, so NVDA, JAWS, and Narrator read them
without any special configuration.

## Before you start

- **Restart afterwards.** Removing kernel drivers and services only fully takes
  effect after a reboot.
- **Avast and AVG use self-protection.** If normal-mode cleanup reports access
  errors, reboot into Windows Safe Mode and run Wixen again.
- Wixen never deletes a driver file while its service is still registered; if
  removal is blocked, the file is reported as skipped rather than deleted,
  because deleting it could stop Windows from starting.

## Requirements

- 64-bit Windows 10 or Windows 11
- Administrator rights

## Removing Wixen

Settings → Apps → Installed apps → **Wixen Uninstaller** → Uninstall.

Want another product supported?
[File an issue.](https://github.com/PratikP1/Wixen-Uninstall/issues)
