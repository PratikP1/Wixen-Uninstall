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

    // Driver images are skipped rather than attempted once their service fails
    // to go away, so every action ends up in exactly one of the two lists.
    assert_eq!(
        report.errors.len() + report.warnings.len(),
        total,
        "Every action should be accounted for as an error or a skip"
    );
    assert!(!report.errors.is_empty());
    assert_eq!(report.actions_succeeded, 0);
    assert!(!report.fully_succeeded());
}

// ─── Boot safety ─────────────────────────────────────────────────────────────

#[test]
fn no_driver_image_is_deleted_when_service_removal_fails() {
    // The failure mode this guards against: self-protection blocks `sc delete`,
    // the driver stays registered, its image is deleted anyway, and Windows
    // will not boot.
    for &product in Product::all() {
        let plan = RemovalPlan::for_product(product);
        let stub = StubExecutor::all_error("Access is denied");
        let report = execute(&plan, &stub);

        let guarded_files = plan
            .file_paths
            .iter()
            .filter(|file| file.guard_service.is_some())
            .count();

        assert_eq!(
            report.warnings.len(),
            guarded_files,
            "{product}: every guarded driver should be skipped, not deleted"
        );
    }
}

#[test]
fn driver_images_are_deleted_once_their_services_are_gone() {
    for &product in Product::all() {
        let plan = RemovalPlan::for_product(product);
        let report = execute(&plan, &StubExecutor::all_removed());

        assert!(
            report.warnings.is_empty(),
            "{product}: nothing should be skipped when every service is removed: {:?}",
            report.warnings
        );
        assert_eq!(report.actions_succeeded, plan.action_count());
    }
}

// ─── Path safety ─────────────────────────────────────────────────────────────

#[test]
fn no_plan_targets_a_system_directory() {
    let never_delete = [
        r"C:\",
        r"C:\Windows",
        r"C:\Windows\System32",
        r"C:\Windows\System32\drivers",
        r"C:\Program Files",
        r"C:\Program Files (x86)",
        r"C:\Program Files\Common Files",
        r"C:\ProgramData",
        r"C:\Users",
    ];

    for &product in Product::all() {
        for file in &RemovalPlan::for_product(product).file_paths {
            let target = file.path.trim_end_matches('\\');
            assert!(
                !never_delete.iter().any(|protected| protected
                    .trim_end_matches('\\')
                    .eq_ignore_ascii_case(target)),
                "{product}: plan would delete the system directory {}",
                file.path
            );
        }
    }
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
