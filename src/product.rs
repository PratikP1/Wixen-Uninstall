//! Product catalogue — the two products we know how to remove.
//!
//! Author: PratikP1

use std::fmt;

/// Every product Wixen is able to uninstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Product {
    McAfee,
    Norton,
}

impl Product {
    /// Human-readable display name shown in menus and log output.
    pub fn display_name(self) -> &'static str {
        match self {
            Product::McAfee => "McAfee Total Protection",
            Product::Norton => "Norton 360 / Norton Security",
        }
    }

    /// All products that Wixen supports, in the order they appear in menus.
    pub fn all() -> &'static [Product] {
        &[Product::McAfee, Product::Norton]
    }

    /// Parse a 1-based menu index entered by the user.
    ///
    /// Returns `None` when the index is out of range or zero.
    pub fn from_menu_index(index: usize) -> Option<Product> {
        Product::all().get(index.wrapping_sub(1)).copied()
    }

    /// Parse from a canonical lowercase slug (`"mcafee"`, `"norton"`).
    ///
    /// Case-insensitive.  Returns `None` for unknown slugs.
    pub fn from_slug(slug: &str) -> Option<Product> {
        match slug.trim().to_lowercase().as_str() {
            "mcafee" => Some(Product::McAfee),
            "norton" => Some(Product::Norton),
            _ => None,
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

    // ── all ──────────────────────────────────────────────────────────────────

    #[test]
    fn all_contains_both_products() {
        let all = Product::all();
        assert!(all.contains(&Product::McAfee));
        assert!(all.contains(&Product::Norton));
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
    fn menu_index_0_is_none() {
        assert_eq!(Product::from_menu_index(0), None);
    }

    #[test]
    fn menu_index_out_of_range_is_none() {
        assert_eq!(Product::from_menu_index(99), None);
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
    fn slug_with_whitespace_is_trimmed() {
        assert_eq!(Product::from_slug("  norton  "), Some(Product::Norton));
    }

    #[test]
    fn unknown_slug_returns_none() {
        assert_eq!(Product::from_slug("avast"), None);
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
    }

    // ── round-trip ───────────────────────────────────────────────────────────

    #[test]
    fn menu_index_round_trip_covers_all_products() {
        for (i, &product) in Product::all().iter().enumerate() {
            assert_eq!(Product::from_menu_index(i + 1), Some(product));
        }
    }
}
