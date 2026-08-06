//! Resume state — what a removal must finish after a normal restart.
//!
//! Author: PratikP1
//!
//! When a locked file is queued for boot-time deletion (see
//! `docs/automated-removal.md`), the removal is not finished, only *suspended*:
//! the file goes at the next boot, but the registry keys and the follow-up
//! sweep still have to run.  Wixen writes this state, registers itself to run
//! once after the restart, and — when the user reboots **normally**, with audio
//! and their screen reader — reads it back and completes the job.
//!
//! The format is plain key/value text so the crate keeps its promise of no
//! serialization dependency.  Parsing is total: a truncated or corrupt file
//! resumes to "nothing pending" rather than panicking, so a bad state file can
//! never cause a blind deletion.

use crate::product::Product;
use std::fmt::Write as _;

// ─── Type ────────────────────────────────────────────────────────────────────

/// Everything needed to finish a removal after a reboot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeState {
    /// The product being removed.
    pub product: Product,
    /// Paths queued for boot-time deletion, to confirm gone and sweep parents.
    pub pending_files: Vec<String>,
    /// Registry keys still to delete.
    pub pending_registry: Vec<String>,
}

const PRODUCT_KEY: &str = "product";
const FILE_KEY: &str = "file";
const REGISTRY_KEY: &str = "registry";
const KEY_VALUE_SEPARATOR: char = '=';

impl ResumeState {
    /// A resume state for `product` with nothing pending yet.
    pub fn new(product: Product) -> Self {
        Self {
            product,
            pending_files: Vec::new(),
            pending_registry: Vec::new(),
        }
    }

    /// Record a file queued for boot-time deletion.
    pub fn add_pending_file(&mut self, path: impl Into<String>) {
        self.pending_files.push(path.into());
    }

    /// Record a registry key still to delete.
    pub fn add_pending_registry(&mut self, key: impl Into<String>) {
        self.pending_registry.push(key.into());
    }

    /// `true` when a restart would have nothing to finish, so no state need be
    /// written and no `RunOnce` entry registered.
    pub fn is_empty(&self) -> bool {
        self.pending_files.is_empty() && self.pending_registry.is_empty()
    }

    /// Serialize to the on-disk key/value form.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        // Values are Windows paths and registry keys: no newlines, so a
        // line-oriented format needs no escaping.
        let _ = writeln!(
            text,
            "{PRODUCT_KEY}{KEY_VALUE_SEPARATOR}{}",
            self.product.slug()
        );
        for file in &self.pending_files {
            let _ = writeln!(text, "{FILE_KEY}{KEY_VALUE_SEPARATOR}{file}");
        }
        for key in &self.pending_registry {
            let _ = writeln!(text, "{REGISTRY_KEY}{KEY_VALUE_SEPARATOR}{key}");
        }
        text
    }

    /// Parse the on-disk form.
    ///
    /// Returns `None` when the product line is missing or names an unknown
    /// product — there is then nothing safe to resume.  Unknown keys and blank
    /// lines are ignored, and a file with no pending work parses to a state
    /// whose [`is_empty`](Self::is_empty) is true rather than an error.
    pub fn parse(text: &str) -> Option<Self> {
        let mut product = None;
        let mut pending_files = Vec::new();
        let mut pending_registry = Vec::new();

        for line in text.lines() {
            let Some((key, value)) = line.split_once(KEY_VALUE_SEPARATOR) else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                PRODUCT_KEY => product = Product::from_slug(value),
                FILE_KEY if !value.is_empty() => pending_files.push(value.to_owned()),
                REGISTRY_KEY if !value.is_empty() => pending_registry.push(value.to_owned()),
                _ => {}
            }
        }

        Some(Self {
            product: product?,
            pending_files,
            pending_registry,
        })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn populated() -> ResumeState {
        let mut state = ResumeState::new(Product::Avast);
        state.add_pending_file(r"C:\Program Files\AVAST Software");
        state.add_pending_file(r"C:\Windows\System32\drivers\aswSnx.sys");
        state.add_pending_registry(r"HKLM\SOFTWARE\AVAST Software");
        state
    }

    // ── round trip ───────────────────────────────────────────────────────────

    #[test]
    fn a_populated_state_round_trips_through_text() {
        let state = populated();
        assert_eq!(ResumeState::parse(&state.to_text()), Some(state));
    }

    #[test]
    fn every_product_round_trips() {
        for &product in Product::all() {
            let mut state = ResumeState::new(product);
            state.add_pending_registry(r"HKLM\SOFTWARE\Example");
            assert_eq!(ResumeState::parse(&state.to_text()), Some(state));
        }
    }

    #[test]
    fn the_serialized_form_names_the_product_by_slug() {
        let text = ResumeState::new(Product::Norton).to_text();
        assert!(text.contains("product=norton"), "{text}");
    }

    #[test]
    fn pending_items_appear_in_the_serialized_form() {
        let text = populated().to_text();
        assert!(text.contains(r"file=C:\Program Files\AVAST Software"));
        assert!(text.contains(r"registry=HKLM\SOFTWARE\AVAST Software"));
    }

    // ── emptiness ────────────────────────────────────────────────────────────

    #[test]
    fn a_fresh_state_has_nothing_pending() {
        assert!(ResumeState::new(Product::McAfee).is_empty());
    }

    #[test]
    fn a_state_with_a_pending_file_is_not_empty() {
        let mut state = ResumeState::new(Product::McAfee);
        state.add_pending_file(r"C:\x");
        assert!(!state.is_empty());
    }

    #[test]
    fn a_state_with_only_a_pending_key_is_not_empty() {
        let mut state = ResumeState::new(Product::McAfee);
        state.add_pending_registry(r"HKLM\SOFTWARE\x");
        assert!(!state.is_empty());
    }

    // ── corrupt / partial input is safe ──────────────────────────────────────

    #[test]
    fn a_file_with_no_product_line_does_not_resume() {
        assert_eq!(
            ResumeState::parse("file=C:\\x\nregistry=HKLM\\SOFTWARE\\x"),
            None,
            "without a product there is nothing safe to resume"
        );
    }

    #[test]
    fn an_unknown_product_does_not_resume() {
        assert_eq!(ResumeState::parse("product=kaspersky\nfile=C:\\x"), None);
    }

    #[test]
    fn empty_input_does_not_resume() {
        assert_eq!(ResumeState::parse(""), None);
        assert_eq!(ResumeState::parse("   \n\n"), None);
    }

    #[test]
    fn garbage_lines_are_ignored_around_a_valid_product() {
        let state = ResumeState::parse(
            "this is not a pair\nproduct=avg\n=leading separator\nfile=C:\\real\nnonsense",
        )
        .expect("a valid product line is present");
        assert_eq!(state.product, Product::Avg);
        assert_eq!(state.pending_files, vec![r"C:\real"]);
        assert!(state.pending_registry.is_empty());
    }

    #[test]
    fn a_product_with_no_pending_work_parses_but_is_empty() {
        let state = ResumeState::parse("product=avast").expect("valid product");
        assert!(
            state.is_empty(),
            "the caller checks is_empty and does no blind deletion"
        );
    }

    #[test]
    fn empty_values_are_dropped_rather_than_stored() {
        let state =
            ResumeState::parse("product=avast\nfile=\nregistry=   ").expect("valid product");
        assert!(state.pending_files.is_empty());
        assert!(state.pending_registry.is_empty());
    }

    #[test]
    fn parsing_arbitrary_input_never_panics() {
        for raw in [
            "\0\0\0",
            "product",
            "==",
            "product==avast",
            &"file=x\n".repeat(1000),
            "PRODUCT=AVAST",
        ] {
            let _ = ResumeState::parse(raw);
        }
    }

    #[test]
    fn the_product_key_is_case_sensitive_but_the_slug_is_not() {
        // The key must be exactly `product`; the slug reuses from_slug, which is
        // case-insensitive, so an upper-case value still resolves.
        assert_eq!(ResumeState::parse("PRODUCT=avast"), None);
        assert_eq!(
            ResumeState::parse("product=AVAST").map(|s| s.product),
            Some(Product::Avast)
        );
    }
}
