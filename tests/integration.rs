//! Integration tests — full pipeline from product selection through execution.
//!
//! Author: PratikP1

use std::io;
use wixen_uninstall_lib::{
    executor::{StubExecutor, execute},
    menu::{MenuChoice, parse_input, run_menu},
    plan::RemovalPlan,
    product::Product,
};

// ─── McAfee end-to-end ────────────────────────────────────────────────────────

#[test]
fn mcafee_full_removal_succeeds_with_stub() {
    let plan = RemovalPlan::for_product(Product::McAfee);
    let stub = StubExecutor::all_removed();
    let report = execute(&plan, &stub);

    assert!(
        report.fully_succeeded(),
        "Expected full success, got errors: {:?}",
        report.errors
    );
    assert_eq!(report.actions_attempted, plan.action_count());
    assert_eq!(report.actions_succeeded, plan.action_count());
}

// ─── Norton end-to-end ────────────────────────────────────────────────────────

#[test]
fn norton_full_removal_succeeds_with_stub() {
    let plan = RemovalPlan::for_product(Product::Norton);
    let stub = StubExecutor::all_removed();
    let report = execute(&plan, &stub);

    assert!(
        report.fully_succeeded(),
        "Expected full success, got errors: {:?}",
        report.errors
    );
    assert_eq!(report.actions_attempted, plan.action_count());
}

#[test]
fn avast_full_removal_succeeds_with_stub() {
    let plan = RemovalPlan::for_product(Product::Avast);
    let stub = StubExecutor::all_removed();
    let report = execute(&plan, &stub);

    assert!(
        report.fully_succeeded(),
        "Expected full success, got errors: {:?}",
        report.errors
    );
    assert_eq!(report.actions_attempted, plan.action_count());
}

#[test]
fn avg_full_removal_succeeds_with_stub() {
    let plan = RemovalPlan::for_product(Product::Avg);
    let stub = StubExecutor::all_removed();
    let report = execute(&plan, &stub);

    assert!(
        report.fully_succeeded(),
        "Expected full success, got errors: {:?}",
        report.errors
    );
    assert_eq!(report.actions_attempted, plan.action_count());
}

// ─── Idempotency ─────────────────────────────────────────────────────────────

#[test]
fn removal_is_idempotent_when_already_uninstalled() {
    // Running against a system where the product is already gone should still
    // succeed — NotFound is treated as success.
    for &product in Product::all() {
        let plan = RemovalPlan::for_product(product);
        let stub = StubExecutor::all_not_found();
        let report = execute(&plan, &stub);
        assert!(
            report.fully_succeeded(),
            "NotFound should be treated as success for {product}"
        );
    }
}

// ─── Error accumulation ───────────────────────────────────────────────────────

#[test]
fn all_errors_are_collected_and_not_short_circuited() {
    let plan = RemovalPlan::for_product(Product::McAfee);
    let total = plan.action_count();
    let stub = StubExecutor::all_error("simulated failure");
    let report = execute(&plan, &stub);

    assert_eq!(
        report.errors.len(),
        total,
        "Every action should produce an error entry"
    );
    assert_eq!(report.actions_succeeded, 0);
}

// ─── Menu → product → plan pipeline ─────────────────────────────────────────

#[test]
fn typing_1_in_menu_leads_to_mcafee_plan() {
    let input = b"1\n";
    let mut reader = io::BufReader::new(input.as_ref());
    let mut output = Vec::new();

    let product = run_menu(&mut reader, &mut output)
        .unwrap()
        .expect("Expected a product");
    let plan = RemovalPlan::for_product(product);

    assert_eq!(plan.product, Product::McAfee);
    assert!(plan.is_non_empty());
}

#[test]
fn typing_2_in_menu_leads_to_norton_plan() {
    let input = b"2\n";
    let mut reader = io::BufReader::new(input.as_ref());
    let mut output = Vec::new();

    let product = run_menu(&mut reader, &mut output)
        .unwrap()
        .expect("Expected a product");
    let plan = RemovalPlan::for_product(product);

    assert_eq!(plan.product, Product::Norton);
    assert!(plan.is_non_empty());
}

#[test]
fn typing_3_in_menu_leads_to_avast_plan() {
    let input = b"3\n";
    let mut reader = io::BufReader::new(input.as_ref());
    let mut output = Vec::new();

    let product = run_menu(&mut reader, &mut output)
        .unwrap()
        .expect("Expected a product");
    let plan = RemovalPlan::for_product(product);

    assert_eq!(plan.product, Product::Avast);
    assert!(plan.is_non_empty());
}

#[test]
fn typing_4_in_menu_leads_to_avg_plan() {
    let input = b"4\n";
    let mut reader = io::BufReader::new(input.as_ref());
    let mut output = Vec::new();

    let product = run_menu(&mut reader, &mut output)
        .unwrap()
        .expect("Expected a product");
    let plan = RemovalPlan::for_product(product);

    assert_eq!(plan.product, Product::Avg);
    assert!(plan.is_non_empty());
}

// ─── Accessibility: no ANSI in menu output ────────────────────────────────────

#[test]
fn menu_output_contains_no_ansi_escapes() {
    let input = b"q\n";
    let mut reader = io::BufReader::new(input.as_ref());
    let mut output = Vec::new();

    run_menu(&mut reader, &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(
        !text.contains('\x1b'),
        "Screen-reader friendly: no ANSI escape codes in output"
    );
}

// ─── parse_input boundary values ─────────────────────────────────────────────

#[test]
fn parse_input_handles_newline_terminated_input() {
    assert_eq!(parse_input("1\n"), MenuChoice::Product(Product::McAfee));
}

#[test]
fn parse_input_handles_crlf_terminated_input() {
    assert_eq!(parse_input("1\r\n"), MenuChoice::Product(Product::McAfee));
}
