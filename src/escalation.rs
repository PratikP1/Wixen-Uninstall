//! The escalation ladder — pure decisions about how hard to try.
//!
//! Author: PratikP1
//!
//! When ordinary deletion is refused, Wixen escalates: reset ownership and the
//! ACL for something denied by permissions, or queue for boot-time deletion for
//! something merely held open.  *Which* remedy fits an outcome, and — crucially
//! — whether the boot-safety guard forbids forcing it at all, is decided here,
//! in the tested core.  The Windows layer carries the decisions out; it never
//! makes them.
//!
//! This is where the driver-guard invariant is enforced for the forceful
//! paths: a kernel driver's image is never taken-ownership-of-and-deleted, nor
//! queued for boot-time deletion, while its service is still registered —
//! because a registered boot-start driver with a missing file stops Windows
//! from booting.  See `docs/automated-removal.md`.

use crate::executor::ActionOutcome;
use crate::plan::FilePath;

// ─── Remedies ────────────────────────────────────────────────────────────────

/// A forceful mechanism for a deletion the ordinary path could not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    /// Take ownership, grant Administrators full control, then delete now.
    /// The remedy for an artifact denied by its ACL.
    TakeOwnership,
    /// Queue the path for deletion at the next boot, then resume after a normal
    /// restart.  The remedy for a file that is merely held open.
    DelayUntilReboot,
}

/// What to do after an attempt to remove a file produced a given outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextStep {
    /// The artifact is gone — removed, or never there.
    Done,
    /// Apply this remedy and try again.
    Escalate(Remedy),
    /// A guarded driver whose service is still present: leave it untouched.
    SkipForSafety(String),
    /// No remedy is left, or none would help; record the failure.
    GiveUp(String),
}

// ─── Why a deletion was refused ──────────────────────────────────────────────

/// The kind of obstruction an error message describes, which decides the
/// remedy: ownership for a permission denial, a delayed delete for a lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Obstruction {
    AccessDenied,
    InUse,
    Other,
}

/// Markers Windows uses for a permission denial.
const ACCESS_DENIED_MARKERS: &[&str] = &["access is denied", "access denied", "denied"];

/// Markers Windows uses when a file is open in another process.
const IN_USE_MARKERS: &[&str] = &[
    "being used by another process",
    "in use",
    "sharing violation",
    "cannot access the file",
    "used by another",
];

fn classify_obstruction(message: &str) -> Obstruction {
    if IN_USE_MARKERS
        .iter()
        .any(|marker| contains_ignore_case(message, marker))
    {
        Obstruction::InUse
    } else if ACCESS_DENIED_MARKERS
        .iter()
        .any(|marker| contains_ignore_case(message, marker))
    {
        Obstruction::AccessDenied
    } else {
        Obstruction::Other
    }
}

// ─── The decision ────────────────────────────────────────────────────────────

/// Decide the next step after removing `file` produced `outcome`.
///
/// `removed_services` is the set of services already gone, `last_remedy` the
/// forceful step just attempted (if any).  The driver guard is consulted first
/// and unconditionally: a guarded driver never escalates, whatever the error.
pub fn next_step(
    file: &FilePath,
    removed_services: &[&str],
    last_remedy: Option<Remedy>,
    outcome: &ActionOutcome,
) -> NextStep {
    if outcome.is_success() {
        return NextStep::Done;
    }

    // The invariant, enforced before any force is considered: while a driver's
    // service survives, its image is off-limits to every forceful path.
    if let Some(service) = file.blocking_guard(removed_services) {
        return NextStep::SkipForSafety(guarded_driver_reason(service));
    }

    match outcome {
        ActionOutcome::Skipped(reason) => NextStep::SkipForSafety(reason.clone()),
        ActionOutcome::Error(message) => escalate_error(message, last_remedy),
        // Success was handled above; these two are unreachable but kept total.
        ActionOutcome::Removed | ActionOutcome::NotFound => NextStep::Done,
    }
}

fn escalate_error(message: &str, last_remedy: Option<Remedy>) -> NextStep {
    match (classify_obstruction(message), last_remedy) {
        // Denied and untouched by force yet: fix the permissions first.
        (Obstruction::AccessDenied, None) => NextStep::Escalate(Remedy::TakeOwnership),
        // Still denied after taking ownership: it must also be open — wait for
        // a boot where it is not.
        (Obstruction::AccessDenied, Some(Remedy::TakeOwnership)) => {
            NextStep::Escalate(Remedy::DelayUntilReboot)
        }
        // Held open: only a reboot frees it, whether or not we own it.
        (Obstruction::InUse, None | Some(Remedy::TakeOwnership)) => {
            NextStep::Escalate(Remedy::DelayUntilReboot)
        }
        // A delayed delete was already queued, or the error is one no remedy
        // addresses: stop rather than loop.
        (_, Some(Remedy::DelayUntilReboot)) | (Obstruction::Other, _) => {
            NextStep::GiveUp(message.to_owned())
        }
    }
}

fn guarded_driver_reason(service: &str) -> String {
    format!(
        "left in place because the {service} service is still present - forcing \
         a registered driver's file can stop Windows from starting"
    )
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    !needle.is_empty()
        && haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn denied() -> ActionOutcome {
        ActionOutcome::Error("Access is denied.".to_owned())
    }

    fn in_use() -> ActionOutcome {
        ActionOutcome::Error(
            "The process cannot access the file because it is being used by another process."
                .to_owned(),
        )
    }

    fn plain_file() -> FilePath {
        FilePath::dir(r"C:\Program Files\McAfee")
    }

    fn guarded_driver() -> FilePath {
        FilePath::driver(r"C:\Windows\System32\drivers\aswSP.sys", "aswSP")
    }

    // ── success ends the ladder ──────────────────────────────────────────────

    #[test]
    fn a_removed_artifact_is_done() {
        assert_eq!(
            next_step(&plain_file(), &[], None, &ActionOutcome::Removed),
            NextStep::Done
        );
        assert_eq!(
            next_step(&plain_file(), &[], None, &ActionOutcome::NotFound),
            NextStep::Done
        );
    }

    // ── the boot-safety invariant on the forceful path ───────────────────────

    #[test]
    fn a_guarded_driver_never_escalates_however_it_failed() {
        let driver = guarded_driver();
        // Denied, in use, after ownership, after a delay — every combination
        // must refuse to force a driver whose service is still present.
        for outcome in [denied(), in_use(), ActionOutcome::Error("locked".into())] {
            for last in [
                None,
                Some(Remedy::TakeOwnership),
                Some(Remedy::DelayUntilReboot),
            ] {
                assert!(
                    matches!(
                        next_step(&driver, &[], last, &outcome),
                        NextStep::SkipForSafety(_)
                    ),
                    "guarded driver escalated: outcome={outcome:?} last={last:?}"
                );
            }
        }
    }

    #[test]
    fn the_guarded_skip_reason_names_the_service_and_the_boot_risk() {
        let NextStep::SkipForSafety(reason) = next_step(&guarded_driver(), &[], None, &denied())
        else {
            panic!("a guarded driver must be skipped");
        };
        assert!(
            reason.contains("aswSP"),
            "the blocking service is named: {reason}"
        );
        assert!(
            reason.contains("Windows from starting"),
            "the reason explains the boot risk: {reason}"
        );
    }

    #[test]
    fn a_driver_escalates_normally_once_its_service_is_gone() {
        // With the service removed, blocking_guard clears and the image is fair
        // game — that is the whole point of removing the service first.
        let driver = guarded_driver();
        assert_eq!(
            next_step(&driver, &["aswSP"], None, &denied()),
            NextStep::Escalate(Remedy::TakeOwnership)
        );
    }

    // ── access denied → ownership → reboot ────────────────────────────────────

    #[test]
    fn a_denied_file_first_has_its_ownership_reset() {
        assert_eq!(
            next_step(&plain_file(), &[], None, &denied()),
            NextStep::Escalate(Remedy::TakeOwnership)
        );
    }

    #[test]
    fn a_file_still_denied_after_ownership_is_deferred_to_reboot() {
        assert_eq!(
            next_step(&plain_file(), &[], Some(Remedy::TakeOwnership), &denied()),
            NextStep::Escalate(Remedy::DelayUntilReboot)
        );
    }

    // ── in use → reboot directly ──────────────────────────────────────────────

    #[test]
    fn a_locked_file_goes_straight_to_a_delayed_delete() {
        assert_eq!(
            next_step(&plain_file(), &[], None, &in_use()),
            NextStep::Escalate(Remedy::DelayUntilReboot)
        );
    }

    #[test]
    fn a_file_locked_even_after_ownership_is_deferred_not_abandoned() {
        assert_eq!(
            next_step(&plain_file(), &[], Some(Remedy::TakeOwnership), &in_use()),
            NextStep::Escalate(Remedy::DelayUntilReboot)
        );
    }

    // ── the ladder terminates ────────────────────────────────────────────────

    #[test]
    fn nothing_escalates_past_a_delayed_delete() {
        for outcome in [denied(), in_use()] {
            assert!(
                matches!(
                    next_step(&plain_file(), &[], Some(Remedy::DelayUntilReboot), &outcome),
                    NextStep::GiveUp(_)
                ),
                "escalated past the final rung for {outcome:?}"
            );
        }
    }

    #[test]
    fn an_error_no_remedy_addresses_is_given_up_immediately() {
        // A disk failure is not fixed by ownership or a reboot.
        let outcome = ActionOutcome::Error("The device is not ready.".to_owned());
        assert!(matches!(
            next_step(&plain_file(), &[], None, &outcome),
            NextStep::GiveUp(_)
        ));
    }

    // ── an already-skipped outcome stays skipped ─────────────────────────────

    #[test]
    fn an_already_skipped_action_is_reported_not_forced() {
        let outcome = ActionOutcome::Skipped("some reason".to_owned());
        assert_eq!(
            next_step(&plain_file(), &[], None, &outcome),
            NextStep::SkipForSafety("some reason".to_owned())
        );
    }

    // ── obstruction classification ───────────────────────────────────────────

    #[test]
    fn obstruction_is_read_from_the_error_text() {
        assert_eq!(
            classify_obstruction("Access is denied."),
            Obstruction::AccessDenied
        );
        assert_eq!(
            classify_obstruction("used by another process"),
            Obstruction::InUse
        );
        assert_eq!(
            classify_obstruction("The device is not ready."),
            Obstruction::Other
        );
    }

    #[test]
    fn a_lock_is_distinguished_from_a_denial_even_when_both_words_appear() {
        // "in use" must win, or a locked file would be sent to take-ownership,
        // which cannot free it.
        assert_eq!(
            classify_obstruction("access to the file is blocked; it is in use"),
            Obstruction::InUse
        );
    }
}
