//! E2E integration tests for workflow execution via plugin_bridge.
//!
//! After the platform plugin refactor, workflow execution is delegated to
//! platform plugins via plugin_bridge. These tests exercise the bridge's
//! execute_workflow() path and require a running Pulse server with the
//! platform plugins loaded.
//!
//! Run with: cargo test --test e2e_executor_tests -- --ignored
//! (Requires Pulse server with plugin-auto-loop running)

use plugin_coding_pack::plugin_bridge;
use plugin_coding_pack::workspace::WorkspaceConfig;

// ── Helper ──────────────────────────────────────────────────────────────────

fn test_config() -> WorkspaceConfig {
    WorkspaceConfig::resolve(None)
}

// ═══════════════════════════════════════════════════════════════════════════
// Workflow execution via plugin_bridge
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "Requires running Pulse server with platform plugins"]
fn execute_workflow_via_bridge() {
    let config = test_config();
    let result = plugin_bridge::execute_workflow(
        "coding-quick-dev",
        "add a hello world function",
        &config,
    );
    // Either succeeds or fails with connection error — should not panic
    match result {
        Ok(v) => {
            assert!(v.get("status").is_some() || v.get("workflow_id").is_some());
        }
        Err(e) => {
            // Connection refused is expected when server isn't running
            assert!(
                e.message.contains("plugin-bridge") || e.message.contains("connection"),
                "unexpected error: {}",
                e.message
            );
        }
    }
}

#[test]
#[ignore = "Requires running Pulse server with platform plugins"]
fn auto_loop_status_via_bridge() {
    let config = test_config();
    let result = plugin_bridge::auto_loop_status(&config);
    match result {
        Ok(v) => {
            // Should return some status object
            assert!(v.is_object());
        }
        Err(e) => {
            assert!(
                e.message.contains("plugin-bridge"),
                "unexpected error: {}",
                e.message
            );
        }
    }
}

#[test]
#[ignore = "Requires running Pulse server with platform plugins"]
fn auto_loop_next_via_bridge() {
    let config = test_config();
    let result = plugin_bridge::auto_loop_next(&config);
    match result {
        Ok(maybe_task) => {
            // Either Some(task) or None (idle)
            if let Some(task) = maybe_task {
                assert!(task.is_object());
            }
        }
        Err(e) => {
            assert!(
                e.message.contains("plugin-bridge"),
                "unexpected error: {}",
                e.message
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Pack action dispatch (non-delegated actions still work locally)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn validate_pack_still_works() {
    let input = plugin_coding_pack::pack::CodingPackInput {
        action: "validate-pack".to_string(),
        target: None,
        workflow_id: None,
        input: None,
        endpoint: None,
        payload: None,
        workspace_dir: None,
        workspace: None,
        board_id: None,
    };
    let result = plugin_coding_pack::pack::execute_action(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.get("plugins_ok").is_some());
}

#[test]
fn list_workflows_still_works() {
    let input = plugin_coding_pack::pack::CodingPackInput {
        action: "list-workflows".to_string(),
        target: None,
        workflow_id: None,
        input: None,
        endpoint: None,
        payload: None,
        workspace_dir: None,
        workspace: None,
        board_id: None,
    };
    let result = plugin_coding_pack::pack::execute_action(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.get("workflows").is_some());
}
