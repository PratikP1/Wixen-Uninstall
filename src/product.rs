//! Product catalogue — the products we know how to remove.
//!
//! Author: PratikP1

use std::fmt;

/// Every product Wixen is able to uninstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Product {
    McAfee,
    Norton,
    Avast,
    Avg,
}

impl Product {
    /// Human-readable display name shown in menus and log output.
    pub fn display_name(self) -> &'static str {
        match self {
            Product::McAfee => "McAfee Total Protection",
            Product::Norton => "Norton 360 / Norton Security",
            Product::Avast => "Avast Antivirus / Avast Premium Security",
            Product::Avg => "AVG AntiVirus / AVG Internet Security",
        }
    }

    /// All products that Wixen supports, in the order they appear in menus.
    pub fn all() -> &'static [Product] {
        &[
            Product::McAfee,
            Product::Norton,
            Product::Avast,
            Product::Avg,
        ]
    }

    /// Parse a 1-based menu index entered by the user.
    ///
    /// Returns `None` when the index is out of range or zero.
    pub fn from_menu_index(index: usize) -> Option<Product> {
        index
            .checked_sub(1)
            .and_then(|menu_index| Product::all().get(menu_index))
            .copied()
    }

    /// The canonical lowercase slug, e.g. `"mcafee"`.
    ///
    /// Stable across releases: it names the product in the resume state written
    /// before a reboot, so it must round-trip with [`Product::from_slug`].
    pub fn slug(self) -> &'static str {
        match self {
            Product::McAfee => "mcafee",
            Product::Norton => "norton",
            Product::Avast => "avast",
            Product::Avg => "avg",
        }
    }

    /// Parse from a canonical lowercase slug (for example `"mcafee"` or `"avg"`).
    ///
    /// Case-insensitive.  Returns `None` for unknown slugs.
    pub fn from_slug(slug: &str) -> Option<Product> {
        let trimmed = slug.trim();

        if trimmed.eq_ignore_ascii_case("mcafee") {
            Some(Product::McAfee)
        } else if trimmed.eq_ignore_ascii_case("norton") {
            Some(Product::Norton)
        } else if trimmed.eq_ignore_ascii_case("avast") {
            Some(Product::Avast)
        } else if trimmed.eq_ignore_ascii_case("avg") {
            Some(Product::Avg)
        } else {
            None
        }
    }

    /// The same-vendor extras this product's cleanup also sweeps up.
    ///
    /// Shown beneath the product name on its selection button, so the choice
    /// can be made without opening the help file.
    pub fn companion_note(self) -> &'static str {
        match self {
            Product::McAfee => "Also removes LiveSafe, WebAdvisor, and SiteAdvisor leftovers.",
            Product::Norton => "Also removes Norton Secure VPN and Norton Utilities leftovers.",
            Product::Avast => "Also removes Avast Secure Browser and Avast Cleanup leftovers.",
            Product::Avg => "Also removes AVG Secure Browser and AVG TuneUp leftovers.",
        }
    }

    /// Extra guidance shown before uninstalling products with self-protection.
    ///
    /// It deliberately does not mention Safe Mode: Windows 10 Safe Mode often
    /// has no audio for a screen reader, so the removal is designed to finish in
    /// normal mode — running the product's own uninstaller and, if needed,
    /// completing after an ordinary restart.
    pub fn pre_removal_note(self) -> Option<&'static str> {
        match self {
            Product::Avast | Product::Avg => Some(
                "This product installs self-protection drivers. Wixen removes them automatically by running the product's own uninstaller and, if anything stays locked, finishing after a normal restart.",
            ),
            Product::McAfee | Product::Norton => None,
        }
    }
}

impl fmt::Display for Product {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── display_name ─────────────────────────────────────────────────────────

    #[test]
    fn mcafee_has_expected_display_name() {
        assert_eq!(Product::McAfee.display_name(), "McAfee Total Protection");
    }

    #[test]
    fn norton_has_expected_display_name() {
        assert!(Product::Norton.display_name().contains("Norton"));
    }

    #[test]
    fn avast_has_expected_display_name() {
        assert!(Product::Avast.display_name().contains("Avast"));
    }

    #[test]
    fn avg_has_expected_display_name() {
        assert!(Product::Avg.display_name().contains("AVG"));
    }

    // ── all ──────────────────────────────────────────────────────────────────

    #[test]
    fn all_contains_supported_products() {
        let all = Product::all();
        assert!(all.contains(&Product::McAfee));
        assert!(all.contains(&Product::Norton));
        assert!(all.contains(&Product::Avast));
        assert!(all.contains(&Product::Avg));
    }

    #[test]
    fn all_is_non_empty() {
        assert!(!Product::all().is_empty());
    }

    // ── from_menu_index ──────────────────────────────────────────────────────

    #[test]
    fn menu_index_1_is_mcafee() {
        assert_eq!(Product::from_menu_index(1), Some(Product::McAfee));
    }

    #[test]
    fn menu_index_2_is_norton() {
        assert_eq!(Product::from_menu_index(2), Some(Product::Norton));
    }

    #[test]
    fn menu_index_3_is_avast() {
        assert_eq!(Product::from_menu_index(3), Some(Product::Avast));
    }

    #[test]
    fn menu_index_4_is_avg() {
        assert_eq!(Product::from_menu_index(4), Some(Product::Avg));
    }

    #[test]
    fn menu_index_0_is_none() {
        assert_eq!(Product::from_menu_index(0), None);
    }

    #[test]
    fn menu_index_out_of_range_is_none() {
        assert_eq!(Product::from_menu_index(99), None);
    }

    #[test]
    fn menu_index_usize_max_is_none() {
        assert_eq!(Product::from_menu_index(usize::MAX), None);
    }

    // ── from_slug ────────────────────────────────────────────────────────────

    #[test]
    fn slug_mcafee_lowercase_works() {
        assert_eq!(Product::from_slug("mcafee"), Some(Product::McAfee));
    }

    #[test]
    fn slug_mcafee_uppercase_works() {
        assert_eq!(Product::from_slug("MCAFEE"), Some(Product::McAfee));
    }

    #[test]
    fn slug_norton_mixed_case_works() {
        assert_eq!(Product::from_slug("Norton"), Some(Product::Norton));
    }

    #[test]
    fn slug_avast_works() {
        assert_eq!(Product::from_slug("avast"), Some(Product::Avast));
    }

    #[test]
    fn slug_avg_mixed_case_works() {
        assert_eq!(Product::from_slug("Avg"), Some(Product::Avg));
    }

    #[test]
    fn slug_with_whitespace_is_trimmed() {
        assert_eq!(Product::from_slug("  norton  "), Some(Product::Norton));
    }

    #[test]
    fn unknown_slug_returns_none() {
        assert_eq!(Product::from_slug("bitdefender"), None);
    }

    #[test]
    fn empty_slug_returns_none() {
        assert_eq!(Product::from_slug(""), None);
    }

    // ── Display ──────────────────────────────────────────────────────────────

    #[test]
    fn display_trait_delegates_to_display_name() {
        assert_eq!(
            format!("{}", Product::McAfee),
            Product::McAfee.display_name()
        );
        assert_eq!(
            format!("{}", Product::Norton),
            Product::Norton.display_name()
        );
        assert_eq!(format!("{}", Product::Avast), Product::Avast.display_name());
        assert_eq!(format!("{}", Product::Avg), Product::Avg.display_name());
    }

    #[test]
    fn companion_notes_name_that_vendor_s_own_extras() {
        assert!(Product::McAfee.companion_note().contains("WebAdvisor"));
        assert!(Product::Norton.companion_note().contains("Secure VPN"));
        assert!(
            Product::Avast
                .companion_note()
                .contains("Avast Secure Browser")
        );
        assert!(Product::Avg.companion_note().contains("AVG TuneUp"));
    }

    #[test]
    fn every_companion_note_is_distinct_and_usable_as_a_button_subtitle() {
        let notes: Vec<&str> = Product::all()
            .iter()
            .map(|product| product.companion_note())
            .collect();

        for (product, note) in Product::all().iter().zip(&notes) {
            assert!(!note.is_empty(), "{product} needs a companion note");
            assert!(note.ends_with('.'), "{product}: {note}");
            assert!(!note.contains('\n'), "{product}: a subtitle is one line");
        }

        let mut unique = notes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), notes.len(), "notes must not be duplicated");
    }

    #[test]
    fn avast_has_a_self_protection_note_that_avoids_safe_mode() {
        let note = Product::Avast
            .pre_removal_note()
            .expect("Avast self-protects, so it needs a note");
        assert!(note.contains("self-protection"));
        assert!(
            note.contains("restart"),
            "the note must point to a normal restart, not Safe Mode: {note}"
        );
        assert!(
            !note.contains("Safe Mode"),
            "a screen-reader user has no audio in Safe Mode: {note}"
        );
    }

    #[test]
    fn mcafee_has_no_self_protection_note() {
        assert!(Product::McAfee.pre_removal_note().is_none());
    }

    // ── round-trip ───────────────────────────────────────────────────────────

    #[test]
    fn menu_index_round_trip_covers_all_products() {
        for (i, &product) in Product::all().iter().enumerate() {
            assert_eq!(Product::from_menu_index(i + 1), Some(product));
        }
    }

    #[test]
    fn slug_round_trips_through_from_slug_for_every_product() {
        // The resume state written before a reboot names the product by slug,
        // so a mismatch here would resume as the wrong product — or not at all.
        for &product in Product::all() {
            assert_eq!(Product::from_slug(product.slug()), Some(product));
        }
    }

    #[test]
    fn slugs_are_distinct() {
        let mut slugs: Vec<&str> = Product::all().iter().map(|p| p.slug()).collect();
        let count = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), count, "two products share a slug");
    }
}
