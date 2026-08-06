# Automated removal — design and implementation plan

**Author:** PratikP1
**Status:** implemented in the pure core and the Windows I/O shims; unverified on
real hardware (see *What is testable, and what is not*)
**Audience:** contributors

## The problem this solves

Some applications actively resist removal. The hardest load a kernel driver
that blocks other processes — Wixen included — from stopping their services or
deleting their files; consumer security suites like Avast and AVG are the
standout case, but the problem is general. The historical workaround is *Safe
Mode*, because such a driver is not loaded there.

Safe Mode is unacceptable for the users Wixen exists to serve. Windows 10's
Safe Mode does not reliably load audio drivers, and without audio a blind user
cannot run Narrator — so "reboot into Safe Mode and run Wixen again" can strand
the very person the tool is built for, with no speech and no way forward.

Every mechanism below runs in **normal Windows**, with the screen reader
running and audio present. Safe Mode is removed as a requirement, not merely
documented around. Nothing here asks the user to operate a vendor's own
settings UI: those interfaces are the accessibility problem, not the solution.

## The escalation ladder

Removal is an ordered sequence of increasingly forceful mechanisms. Each step
is attempted only for the artifacts the previous step could not remove, so the
cheapest, safest action always runs first and force is applied only where it is
needed.

```
0.  Elevate to SYSTEM          once, up front — many steps below need it
1.  Vendor silent uninstall    let the product remove itself; it can bypass
                               its own self-protection
2.  Standard sweep             Wixen's existing plan: tasks, services, files,
                               registry (unchanged, still driver-guarded)
3.  Take ownership + reset ACL  for artifacts denied by permissions rather
                               than by a driver
4.  Delayed deletion           schedule what is still locked for removal at the
                               next boot, then resume automatically
5.  Report                     what went, what is pending until restart, what
                               genuinely failed
```

A run succeeds without ever leaving normal mode when steps 1–4 between them
clear everything. When something survives, the report says so plainly and points
to a normal restart and a re-run — it never sends the user to Safe Mode, which
these users cannot hear.

### Step 0 — Elevate to SYSTEM

Administrator is not always enough. Some services and files are ACL'd against
Administrators but not against `NT AUTHORITY\SYSTEM`, and some vendor
uninstallers (step 1) run *truly* silently only under SYSTEM — otherwise they
can raise their own prompt, which is itself an accessibility trap.

Wixen re-launches itself as SYSTEM through a transient scheduled task registered
to run as `NT AUTHORITY\SYSTEM` (`schtasks /RU SYSTEM /RL HIGHEST`), runs it
once, and deletes the task — the standard technique, needing only the
Administrator token Wixen already requires (see `system_exec.rs`).

A SYSTEM process runs in **session 0**, which has no desktop: it can show no
menu, no progress dialog, and reach no screen reader. So the split is strict.
The interactive Administrator process owns every screen — selection,
confirmation, a "working" dialog, the report. The SYSTEM process runs
**headless**: it executes steps 1–4, registers any boot-time resume, and writes
the report to a file under `%ProgramData%\Wixen\` that the interactive process
reads back and shows. While it runs, the interactive process shows an
indeterminate "removing…" dialog, because no per-item progress crosses the
process boundary.

SYSTEM is an amplifier, never a precondition. If any step of the relaunch fails
— the scheduled task cannot be created or run, or it produces no readable result
— the interactive process runs the removal in-process under Administrator, with
the live progress bar, exactly as it does without SYSTEM. The re-launched
instance takes an `--execute <product>` branch that only runs the removal and
never opens the menu, so a SYSTEM run can never spawn another.

### Step 1 — Vendor silent uninstall

Self-protection cannot block the product's *own* uninstaller, or the product
would be unremovable. So the most reliable way past self-protection is to ask
the product to remove itself.

Wixen already knows each product's `HKLM\…\Uninstall\<name>` keys — it deletes
them. Before deleting, it **reads** `QuietUninstallString`, falling back to
`UninstallString`, and parses it into a program and argument list (see *Parsing
uninstall strings*). It then runs that command **only when it is already
silent**: a `QuietUninstallString`, an MSI string normalized to a silent
`msiexec /x … /qn /norestart`, or an `UninstallString` that already carries a
known silent switch. Wixen never *appends* a guessed silent switch — a wrong
guess can leave the uninstaller blocking on a dialog a screen-reader user cannot
dismiss, the very trap this feature removes. A string that is not already silent
is skipped, and step 2 sweeps whatever it would have removed.

- **Avast / AVG.** `…\Avast\setup\Instup.exe /instop:uninstall /silent`.
  Full silence also wants `SilentUninstallEnabled=1` in the `Common` section of
  `…\setup\stats.ini`; Wixen writes that line itself — a plain INI edit, no UI.
- **Norton.** Uses its own removal path via the registered string; NRnR / the
  Norton Remove and Reinstall flow is the vendor-supported route.
- **McAfee.** No public silent switch on the product; MCPR is the vendor tool.
  Wixen runs the registered string where present and relies on later steps
  otherwise.

The vendor uninstaller is best-effort. Whatever it leaves behind — and these
tools are notorious for leaving plenty — is swept by step 2, which is Wixen's
existing strength.

### Step 2 — Standard sweep

Unchanged. The existing `execute` pipeline: scheduled tasks, then services,
then files, then registry, in that order, **with the driver-image guard
intact**. See *The driver-guard invariant*.

### Step 3 — Take ownership and reset ACLs

A large share of McAfee and Norton leftovers are not driver-protected at all —
they are ordinary files and keys whose ACL denies Administrators. These need no
reboot and no driver interaction:

1. Enable `SeTakeOwnershipPrivilege` / `SeRestorePrivilege` on Wixen's token.
2. Set the owner to the Administrators group.
3. Grant Administrators full control (reset the DACL).
4. Retry the deletion.

Applied only to artifacts that failed step 2 with an access-denied error, so
the forceful ACL rewrite touches nothing that plain deletion already handled.

### Step 4 — Delayed deletion, then resume after a normal restart

A file held open by a running process cannot be deleted now, but Windows can
delete it during early boot, before the process starts:

```
MoveFileEx(path, NULL, MOVEFILE_DELAY_UNTIL_REBOOT)
```

This queues the path in `PendingFileRenameOperations`; the session manager
deletes it at next boot. It requires no Safe Mode.

Deletion alone is not enough — the registry keys and the follow-up sweep still
need to run after the file is gone. So Wixen registers a **RunOnce** entry that
re-launches it after the next restart, carrying the resume state (see *Resume
across a reboot*). The user restarts **normally**: audio present, screen reader
running. Wixen finishes the sweep itself.

This is the accessible replacement for Safe Mode: *restart when convenient*
rather than *reboot into a mode where you cannot hear*.

**Boot-safety still governs this step.** Scheduling a driver image for
delayed deletion while its service is still registered would reintroduce the
unbootable-Windows failure, merely deferred by one reboot. The guard from
*The driver-guard invariant* applies to the delayed path exactly as it does to
immediate deletion: a driver image is never queued for boot-time removal while
its service survives.

## The driver-guard invariant

Wixen already refuses to delete a kernel driver's `.sys` image while that
driver's service is still registered, because Windows will not boot if a
registered boot-start driver's file is missing (`executor.rs`,
`FilePath::blocking_guard`). Every new mechanism must preserve this:

> A driver image is removed — now **or at next boot** — only after its service
> has been successfully removed.

Concretely:

- Step 3 may reset a driver image's ACL, but must not delete it while guarded.
- Step 4 must not add a guarded driver image to
  `PendingFileRenameOperations`.

The invariant is enforced in the pure core (`FilePath::blocking_guard` already
exists and is tested), and every escalation step consults it before acting on a
`.sys` file. New tests assert that no guarded driver reaches either the ACL
reset or the delayed-deletion queue while its service is present.

## Resume across a reboot

When step 4 queues anything for boot-time deletion, the run is not finished —
it is *suspended*. The state needed to finish is:

- the product being removed,
- which artifacts are already done (so they are not re-attempted), and
- which are pending until the restart.

This is written to a small state file under `%ProgramData%\Wixen\` and pointed
at by a `RunOnce` registry value. After the user restarts normally, Windows
runs Wixen once with a resume flag; Wixen reads the state, completes the
registry cleanup and the follow-up sweep, reports, and clears both the state
file and the RunOnce entry.

The state format is plain, hand-written key/value text — no serialization
dependency, consistent with the crate's no-dependencies promise — and its
parser round-trips under test, including a corrupt-file case that resumes to a
safe "nothing pending" rather than panicking.

## Parsing uninstall strings

A registry uninstall string is a command line, not a path. It must be split
into a program and an argument vector, which is the fiddly, security-relevant
part and therefore lives in the tested pure core:

- Quoted program path with spaces: `"C:\Program Files\…\Instup.exe" /x`.
- Unquoted program path (legacy): `C:\PROGRA~1\…\uninst.exe /S`.
- MSI product codes: `MsiExec.exe /X{GUID}` → normalized to a silent
  `msiexec /x {GUID} /qn` form.
- Appending the correct silent switch per uninstaller family only when the
  string does not already request silence, so Wixen never double-passes a flag
  or runs an uninstaller that then blocks on a dialog.

Parsing never executes anything; it returns a structured command that the
Windows layer runs. Malformed input yields a typed error, not a panic, and is
fuzzed alongside the existing path resolver.

## What is testable, and what is not

Consistent with the rest of the codebase, the split is deliberate:

**Pure core — unit-tested on Linux, mutation-tested, and fuzzed where it
parses untrusted input:**

- uninstall-string parsing → program + args,
- the escalation ladder: given a per-artifact outcome, what to try next,
- the delayed-deletion decision, including the driver guard,
- resume-state read/write and its corrupt-input handling,
- `stats.ini` editing,
- self-protection detection from error text.

**Windows-only I/O — behind the `Executor` trait, stubbed in tests, compiled
and linted on `x86_64-pc-windows-msvc`, and smoke-tested by the CI launch
check:**

- `MoveFileEx`, take-ownership/`SetNamedSecurityInfo`, `RunOnce` registration,
- running as SYSTEM via a transient scheduled task,
- invoking the vendor uninstaller.

**The honest limit.** CI proves this code compiles, links, and starts. It
cannot prove that Avast's uninstaller is actually invoked correctly, that ACL
resets defeat a real McAfee leftover, or that resume-after-reboot fires — those
require a Windows machine with a real AV installed. Each escalation step is
therefore built so its *decision* is tested in the core and only its *effect*
is unverified until a maintainer runs it on real hardware. This limitation is
called out in the release notes for the version that ships it, and the feature
is not described as verified until it has been.

## Implementation order

Each step is a red/green TDD increment. Earlier steps deliver value alone, so
the work can stop at any boundary and still improve on Safe Mode.

1. **Uninstall-string parsing** (pure). Types, parser, tests, fuzz target.
2. **Escalation ladder** (pure). Model per-artifact outcomes and the
   next-action decision; assert the driver guard holds at every step.
3. **Resume state** (pure). Read/write, round-trip, corrupt-input safety.
4. **`Executor` trait extension** (I/O boundary). New methods for run-vendor,
   take-ownership, delayed-delete, register-resume; stub implementations for
   tests; real implementations gated to Windows.
5. **Orchestration.** Wire the ladder into `execute`, preserving order and the
   guard; progress and reporting extended for the new phases.
6. **Docs and release notes.** Rewrite the Safe Mode guidance around the
   automated flow; state the testing limit plainly.

## Failure modes and how each is handled

| Failure | Handling |
|---|---|
| SYSTEM elevation refused | continue under Administrator; SYSTEM is an amplifier, not a precondition |
| Vendor uninstaller missing / string absent | skip step 1; the standard sweep still runs |
| Vendor uninstaller blocks on a dialog | run under SYSTEM to keep it silent; if it still blocks, time out and fall through to later steps |
| Take-ownership denied | record the artifact as failed; do not loop |
| Delayed deletion of a **guarded** driver | refused by the invariant; reported as skipped, never queued |
| Corrupt resume state after reboot | parse to "nothing pending"; report and clear, never panic |
| RunOnce fires but the state file is gone | no-op resume; nothing is deleted blind |
| User never restarts | pending deletions simply do not happen; the report already told them a restart is required |
| Self-protection intact **and** no usable vendor uninstaller | the protected artifacts may survive every automated step; the report lists exactly what remains and points to a restart-and-retry, never to Safe Mode. This is the accepted floor: an inaccessible instruction is worse than an honest, incomplete removal |

## Where this can still fall short

The ladder retires Safe Mode for the common cases — a working vendor
uninstaller (step 1) disarms self-protection from the inside, and most stubborn
leftovers are permission-denied rather than driver-protected (step 3) or merely
locked (step 4). The residual hard case is a product whose self-protection is
fully intact *and* whose own uninstaller cannot be found or run. There, a normal
restart may not release the guarded driver, and Wixen deliberately stops rather
than delete a registered driver's image (which would break boot) or send the
user somewhere they cannot hear. The removal is reported as incomplete, with the
survivors named. Closing this last gap is what step 0 (running as SYSTEM, so the
vendor uninstaller runs truly silently) is for; until that is verified on real
hardware it is documented as an amplifier, not a guarantee.
