# Removing stubborn products without Safe Mode

**Author:** PratikP1

## The problem

Consumer security suites install a kernel driver whose job is to stop anything
from removing them. Avast and AVG call it Self-Defense; Norton calls it Product
Tamper Protection. While that driver is loaded it blocks Wixen from stopping the
product's services, deleting its files, and deleting its registry keys — no
matter how privileged Wixen is, because the block happens in the kernel rather
than through file permissions.

Safe Mode works around this only because the driver is not loaded there.

**Safe Mode is not an acceptable answer for this project.** Windows 10 does not
reliably load audio drivers in Safe Mode, so a blind user who reboots into it
may have no speech at all: Narrator cannot be started, and neither can NVDA or
JAWS. Telling someone to go somewhere they cannot hear is not an instruction,
it is an abandonment.

Nor is "turn Self-Defense off first". That means navigating the vendor's own
settings interface, and the inaccessibility of those interfaces is a large part
of why this tool exists.

Whatever Wixen does about self-protection, it has to do by itself.

## The approach

One escalation ladder, tried per artifact, plus two bookends.

```
    ┌─ before ────────────────────────────────────────────┐
    │  0. Run the vendor's own uninstaller, silently      │
    └─────────────────────────────────────────────────────┘
                              ↓
    ┌─ per artifact ──────────────────────────────────────┐
    │  1. Delete it                                       │
    │  2. Take ownership, reset the ACL, delete it again  │
    │  3. Hand it to Windows to delete during next boot   │
    └─────────────────────────────────────────────────────┘
                              ↓
    ┌─ after ─────────────────────────────────────────────┐
    │  4. If anything was deferred, arrange to finish      │
    │     automatically after a normal restart            │
    └─────────────────────────────────────────────────────┘
```

### 0. The vendor's own uninstaller

Self-protection cannot block the vendor's uninstaller — if it did, the product
would be unremovable and would never have shipped. So the one program on the
machine guaranteed to be able to switch off Self-Defense is the one the vendor
already installed.

Wixen reads `UninstallString` or `QuietUninstallString` from the product's
`HKLM\…\CurrentVersion\Uninstall\…` key — the same keys it later deletes — and
runs it with the silent switches for that vendor. For Avast and AVG that is

    "…\Avast\setup\Instup.exe" /instop:uninstall /silent

Wixen's own sweep then removes what the vendor uninstaller leaves behind, which
is the job it was written for in the first place.

This is the step that actually retires Safe Mode. The rest widen coverage.

### 1–2. Direct deletion, then ownership

Not every refusal comes from a self-protection driver. A good share of stubborn
leftovers — McAfee and Norton registry keys especially — are simply owned by
`SYSTEM` or `TrustedInstaller` with an ACL that excludes Administrators. Those
need no driver bypass at all: take ownership, grant ourselves control, delete.

Trying this only *after* a direct delete has failed keeps the common case fast
and avoids rewriting security descriptors we never needed to touch.

### 3. Deletion during the next boot

`MoveFileEx(path, NULL, MOVEFILE_DELAY_UNTIL_REBOOT)` records the path in

    HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\PendingFileRenameOperations

and `smss.exe` deletes it early in the next boot, before the desktop exists and
before anything can hold it open. This clears files that are merely *in use* —
a running process holding a handle — which no amount of privilege can delete
while Windows is up.

It is weaker against an active self-protection driver, which may already be
loaded by the time `smss` runs. It is listed last for that reason: it is the
fallback, not the plan.

### 4. Finishing after a normal restart

When anything is deferred, Wixen writes a `RunOnce` entry so that it resumes
automatically after the next restart and sweeps what is left.

The restart is an **ordinary** restart. Audio works, the screen reader starts as
usual, and the user does not have to do anything except restart when convenient.
That is the whole point: it is the accessible replacement for Safe Mode.

`RunOnce` is self-deleting by definition, so a resume that never happens leaves
nothing behind.

## Running as SYSTEM

Administrator is not the highest privilege on Windows, and two things want more
than it:

- Avast's uninstaller only runs *fully* silently under the SYSTEM account.
  Otherwise it raises its own prompt — a vendor dialog, with all the
  accessibility problems that implies.
- `TrustedInstaller`-owned keys resist Administrator but yield to SYSTEM.

Wixen escalates by registering a transient scheduled task that runs as SYSTEM,
starting it, and removing it — the same technique PsExec uses. If that fails,
Wixen carries on as Administrator; every step degrades to "reported as an
error" rather than "crashes" or "silently does nothing".

## Safety invariants

These hold regardless of how far the ladder is climbed. They are enforced by
tests, not by care.

1. **A driver image is never deleted while its service is still registered.**
   Windows will not boot when a registered boot-start driver's file is missing.
   This already gates direct deletion; it gates boot-scheduled deletion *more*
   strictly, because a deletion scheduled for early boot happens when nothing
   is left to stop it. A guarded driver whose service survived is skipped at
   every rung of the ladder, including the last.

2. **Escalation only ever follows a real failure.** `NotFound` means the
   artifact is already gone, and `Skipped` means Wixen refused on purpose.
   Neither may escalate — taking ownership of something that is not there, or
   scheduling a deliberately-spared driver for boot deletion, would turn a
   correct outcome into a dangerous one.

3. **Nothing outside the plan is touched.** Ownership changes and boot-time
   deletions apply only to paths that already passed the validation in
   `paths.rs`: absolute, at least two levels below the drive root, and never a
   system directory.

4. **The vendor uninstaller is found, never guessed.** Wixen runs the command
   the product itself registered. It does not search the disk for something
   that looks like an uninstaller, and it does not download anything.

## What this does not solve

If Self-Defense is active *and* the vendor uninstaller fails or is missing,
Wixen still cannot remove the protected artifacts, and Safe Mode remains the
only recourse. The report says so explicitly rather than pretending otherwise,
and lists exactly what survived.

## Testing

The decisions live in cross-platform code and are tested on every platform: the
command-line parsing, the choice of silent switches, when to escalate and when
to refuse, the resume decision, and every safety invariant above.

The Windows-only shims — reading the registry, rewriting a security descriptor,
`MoveFileEx`, the `RunOnce` write, the scheduled task — are kept as thin as they
can be, because they are the part CI can compile but not exercise.

**This feature cannot be validated by CI alone.** Its entire purpose is to work
against a real product on a real machine. Before any release that includes it,
it needs a run against an actual Avast or AVG install, with a screen reader
active, on h