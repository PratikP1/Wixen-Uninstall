//! Accessible CLI menu — keyboard-only, screen-reader friendly.
//!
//! Author: PratikP1
//!
//! The menu renders plain numbered text (no ANSI colour codes that could
//! confuse screen readers) and writes prompts to stdout so assistive
//! technology can intercept them.

use crate::product::Product;
use std::io::{self, BufRead, Write};

// ─── Prompt text ─────────────────────────────────────────────────────────────

/// The header printed above the product list.
pub const HEADER: &str = "Wixen Uninstaller — select a product to remove:";

/// The prompt shown after the list.
pub const PROMPT: &str = "Enter number and press Enter (or 'q' to quit): ";

// ─── Menu rendering ──────────────────────────────────────────────────────────

/// Write the product selection menu to `out`.
pub fn render_menu(out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "{HEADER}")?;
    writeln!(out)?;
    for (i, product) in Product::all().iter().enumerate() {
        writeln!(out, "  {}. {}", i + 1, product.display_name())?;
    }
    writeln!(out)?;
    write!(out, "{PROMPT}")?;
    out.flush()
}

// ─── Input parsing ────────────────────────────────────────────────────────────

/// User's answer from the menu prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuChoice {
    /// The user selected a valid product.
    Product(Product),
    /// The user typed `q` or `Q`.
    Quit,
    /// Input was not a valid choice; carry the raw input for the error message.
    Invalid(String),
}

/// Parse a single line of user input into a `MenuChoice`.
pub fn parse_input(raw: &str) -> MenuChoice {
    let trimmed = raw.trim();

    if trimmed.eq_ignore_ascii_case("q") {
        return MenuChoice::Quit;
    }

    if let Ok(n) = trimmed.parse::<usize>()
        && let Some(p) = Product::from_menu_index(n)
    {
        return MenuChoice::Product(p);
    }

    // Also accept product slugs for script/accessibility tool usage.
    if let Some(p) = Product::from_slug(trimmed) {
        return MenuChoice::Product(p);
    }

    MenuChoice::Invalid(trimmed.to_owned())
}

// ─── Interactive loop ─────────────────────────────────────────────────────────

/// Run the interactive menu loop, reading from `input` and writing to `output`.
///
/// Returns the chosen `Product`, or `None` if the user quit.
pub fn run_menu(input: &mut dyn BufRead, output: &mut dyn Write) -> io::Result<Option<Product>> {
    loop {
        render_menu(output)?;

        let mut line = String::new();
        let bytes_read = input.read_line(&mut line)?;

        // EOF — treat like quit.
        if bytes_read == 0 {
            return Ok(None);
        }

        match parse_input(&line) {
            MenuChoice::Product(p) => return Ok(Some(p)),
            MenuChoice::Quit => return Ok(None),
            MenuChoice::Invalid(s) => {
                writeln!(
                    output,
                    "\nInvalid choice \"{s}\". Please enter a number from 1 to {}.",
                    Product::all().len()
                )?;
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── render_menu ──────────────────────────────────────────────────────────

    #[test]
    fn render_menu_contains_header() {
        let mut buf = Vec::new();
        render_menu(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains(HEADER));
    }

    #[test]
    fn render_menu_lists_all_products() {
        let mut buf = Vec::new();
        render_menu(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        for product in Product::all() {
            assert!(
                text.contains(product.display_name()),
                "Menu should list: {}",
                product.display_name()
            );
        }
    }

    #[test]
    fn render_menu_contains_numbered_entries() {
        let mut buf = Vec::new();
        render_menu(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("1."));
        assert!(text.contains("2."));
    }

    #[test]
    fn render_menu_contains_prompt() {
        let mut buf = Vec::new();
        render_menu(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains(PROMPT));
    }

    #[test]
    fn render_menu_has_no_ansi_escape_codes() {
        let mut buf = Vec::new();
        render_menu(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            !text.contains('\x1b'),
            "Menu must not contain ANSI escape codes"
        );
    }

    // ── parse_input ───────────────────────────────────────────────────────────

    #[test]
    fn parse_1_selects_mcafee() {
        assert_eq!(parse_input("1"), MenuChoice::Product(Product::McAfee));
    }

    #[test]
    fn parse_2_selects_norton() {
        assert_eq!(parse_input("2"), MenuChoice::Product(Product::Norton));
    }

    #[test]
    fn parse_3_selects_avast() {
        assert_eq!(parse_input("3"), MenuChoice::Product(Product::Avast));
    }

    #[test]
    fn parse_4_selects_avg() {
        assert_eq!(parse_input("4"), MenuChoice::Product(Product::Avg));
    }

    #[test]
    fn parse_q_lowercase_quits() {
        assert_eq!(parse_input("q"), MenuChoice::Quit);
    }

    #[test]
    fn parse_q_uppercase_quits() {
        assert_eq!(parse_input("Q"), MenuChoice::Quit);
    }

    #[test]
    fn parse_slug_mcafee_works() {
        assert_eq!(parse_input("mcafee"), MenuChoice::Product(Product::McAfee));
    }

    #[test]
    fn parse_slug_norton_works() {
        assert_eq!(parse_input("norton"), MenuChoice::Product(Product::Norton));
    }

    #[test]
    fn parse_slug_avast_works() {
        assert_eq!(parse_input("avast"), MenuChoice::Product(Product::Avast));
    }

    #[test]
    fn parse_slug_avg_works() {
        assert_eq!(parse_input("avg"), MenuChoice::Product(Product::Avg));
    }

    #[test]
    fn parse_whitespace_around_input_trimmed() {
        assert_eq!(parse_input("  1  "), MenuChoice::Product(Product::McAfee));
    }

    #[test]
    fn parse_zero_is_invalid() {
        assert_eq!(parse_input("0"), MenuChoice::Invalid("0".into()));
    }

    #[test]
    fn parse_out_of_range_is_invalid() {
        assert_eq!(parse_input("99"), MenuChoice::Invalid("99".into()));
    }

    #[test]
    fn parse_garbage_is_invalid() {
        assert_eq!(parse_input("foobar"), MenuChoice::Invalid("foobar".into()));
    }

    #[test]
    fn parse_empty_string_is_invalid() {
        assert_eq!(parse_input(""), MenuChoice::Invalid("".into()));
    }

    // ── run_menu ─────────────────────────────────────────────────────────────

    #[test]
    fn run_menu_returns_mcafee_for_input_1() {
        let input = b"1\n";
        let mut reader = io::BufReader::new(input.as_ref());
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, Some(Product::McAfee));
    }

    #[test]
    fn run_menu_returns_norton_for_input_2() {
        let input = b"2\n";
        let mut reader = io::BufReader::new(input.as_ref());
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, Some(Product::Norton));
    }

    #[test]
    fn run_menu_returns_avast_for_input_3() {
        let input = b"3\n";
        let mut reader = io::BufReader::new(input.as_ref());
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, Some(Product::Avast));
    }

    #[test]
    fn run_menu_returns_avg_for_input_4() {
        let input = b"4\n";
        let mut reader = io::BufReader::new(input.as_ref());
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, Some(Product::Avg));
    }

    #[test]
    fn run_menu_returns_none_for_q() {
        let input = b"q\n";
        let mut reader = io::BufReader::new(input.as_ref());
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn run_menu_retries_on_invalid_then_selects() {
        // First line is invalid, second is valid.
        let input = b"xyz\n2\n";
        let mut reader = io::BufReader::new(input.as_ref());
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, Some(Product::Norton));
        // The output should contain the "Invalid choice" message.
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Invalid choice"));
    }

    #[test]
    fn run_menu_returns_none_on_eof() {
        let input: &[u8] = b"";
        let mut reader = io::BufReader::new(input);
        let mut output = Vec::new();
        let result = run_menu(&mut reader, &mut output).unwrap();
        assert_eq!(result, None);
    }
}
