# Security Policy

## Reporting a vulnerability

Please report security issues privately through
[GitHub Security Advisories](https://github.com/PratikP1/Wixen-Uninstall/security/advisories/new)
rather than opening a public issue. Include the Wixen version, your Windows
version, and the steps to reproduce.

You can expect an initial response within a week.

## Supported versions

Only the latest release receives fixes.

## What Wixen does, and why it needs Administrator

Wixen deletes files, registry keys, Windows services, and scheduled tasks that
belong to consumer security suites. Every one of those operations needs an
elevated token, so the executable carries a manifest requesting Administrator
and Windows shows a UAC prompt when you launch it.

That is a lot of privilege, so the design deliberately constrains it:

- **No dynamic targets.** The list of things Wixen will delete is compiled into
  the binary. Nothing is read from a config file, the network, or the command
  line, so there is no input that can redirect a deletion.
- **Validated paths.** Every path is checked before it reaches the executor. A
  target must be an absolute `X:\…` path at least two levels below the drive
  root, with no `.` or `..` segments, and must not be a drive root, Windows,
  System32, System32\drivers, Program Files, Program Files (x86), a Common
  Files directory, ProgramData, or Users. Failing any of those checks drops the
  entry instead of deleting it.
- **Driver images are guarded.** A `.sys` file is only deleted after its
  service has been successfully removed. Deleting the image of a still
  registered boot-start driver can leave Windows unable to boot, so when
  service removal fails, the file is left alone and reported as skipped.
- **Help opens from the install directory only.** The bundled help file is
  loaded from the directory holding the executable, never from an ancestor
  directory. The root of the system drive is writable by authenticated users on
  a default Windows install, so searching upwards would let any user choose
  what an elevated browser opens.

## Unsigned binaries

Releases are not code-signed, so Windows SmartScreen will warn you the first
time you run the installer. Verify the download against the `.sha256` file
published alongside it on the
[Releases](https://github.com/PratikP1/Wixen-Uninstall/releases) page:

```powershell
Get-FileHash -Algorithm SHA256 .\WixenUninstaller-Setup-0.4.0.exe
```

Only download Wixen from the GitHub Releases page of this repository.
