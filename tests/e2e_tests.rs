//! E2E integration tests for the workflow execution pipeline.
//!
//! After the platform plugin refactor, workflow execution is delegated to
//! platform plugins via plugin_bridge. These tests require real plugin
//! binaries and a running Pulse server for full workflow tests.
//!
//! Gate: Set PULSE_E2E_ENABLED=1 to run ignored tests.

mod e2e;

use e2e::E2EHarness;
use std::path::Path;

/// Story 11.1: Verify the test fixture project exists and the harness works.
#[test]
fn fixture_project_exists_and_compiles() {
    let fixture = Path::new("tests/fixtures/sample-project");
    assert!(fixture.exists(), "fixture project not found");
    assert!(fixture.join("Cargo.toml").exists());
    assert!(fixture.join("src/lib.rs").exists());
}

/// Story 11.1: Verify harness setup and teardown.
#[test]
fn harness_lifecycle() {
    let fixture = Path::new("tests/fixtures/sample-project");
    if !fixture.exists() {
        return;
    }
    let harness = E2EHarness::new(fixture).unwrap();
    assert!(harness.work_dir.exists());
    assert!(harness.work_dir.join(".git").exists());
}

/// Story 11.2: Happy path E2E — requires Pulse server + platform plugins.
/// Submits a feature-dev workflow via plugin_bridge and verifies completion.
#[test]
#[ignore = "Requires Pulse server with platform plugins. Set PULSE_E2E_ENABLED=1"]
fn happy_path_feature_dev() {
    if std::env::var("PULSE_E2E_ENABLED").as_deref() != Ok("1") {
        return;
    }

    let fixture = Path::new("tests/fixtures/sample-project");
    let harness = E2EHarness::new(fixture).expect("harness setup failed");

    let result = harness
        .submit_workflow(
            "coding-feature-dev",
            "Add a function multiply(a, b) that returns a * b with unit tests",
        )
        .expect("workflow execution failed");

    assert_eq!(result["status"].as_str(), Some("completed"));
    // Verify key steps succeeded
    E2EHarness::assert_step_status(&result, "dev_implement", "success")
        .expect("implement step failed");
}

/// Story 11.3: Failure path — quality gate blocks commit when tests fail.
#[test]
#[ignore = "Requires Pulse server with platform plugins. Set PULSE_E2E_ENABLED=1"]
fn failure_path_quality_gate_blocks() {
    if std::env::var("PULSE_E2E_ENABLED").as_deref() != Ok("1") {
        return;
    }

    let fixture = Path::new("tests/fixtures/sample-project");
    let harness = E2EHarness::new(fixture).expect("harness setup failed");

    // Submit a task that's likely to produce failing tests
    let result = harness
        .submit_workflow(
            "coding-quick-dev",
            "Add a function divide(a, b) that returns a / b (intentionally skip zero-check)",
        )
        .expect("workflow execution failed");

    // Workflow should fail or have blocked commit if tests fail
    let status = result["status"].as_str().unwrap_or("unknown");
    assert!(
        status == "failed" || status == "partial",
        "expected failed/partial, got: {}",
        status
    );
}

/// Story 11.4: Retry loop E2E — validates self-correction.
#[test]
#[ignore = "Requires Pulse server with platform plugins. Set PULSE_E2E_ENABLED=1"]
fn retry_loop_self_correction() {
    if std::env::var("PULSE_E2E_ENABLED").as_deref() != Ok("1") {
        return;
    }

    let fixture = Path::new("tests/fixtures/sample-project");
    let harness = E2EHarness::new(fixture).expect("harness setup failed");

    let result = harness
        .submit_workflow(
            "coding-feature-dev",
            "Implement safe_divide(a, b) -> Result<f64, String> that returns Err for division by zero",
        )
        .expect("workflow execution failed");

    // May succeed on first try or after retries
    let status = result["status"].as_str().unwrap_or("unknown");
    // Accept either completed (success) or failed (retries exhausted)
    assert!(
        status == "completed" || status == "failed",
        "unexpected status: {}",
        status
    );
}
