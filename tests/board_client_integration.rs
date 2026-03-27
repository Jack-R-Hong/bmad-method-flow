//! Integration tests for board_client — requires Pulse server + plugin-board running.
//!
//! Run: cargo test --test board_client_integration
//! (Pulse server with plugin-board must be running on localhost:8080)

use plugin_coding_pack::board_client;

const TEST_WORKSPACE: &str = "Default";

fn require_board_plugin() {
    if !board_client::is_available() {
        panic!(
            "plugin-board not available at localhost:8080. \
             Start Pulse server with plugin-board before running these tests."
        );
    }
}

fn list_all() -> Vec<board_client::Assignment> {
    board_client::list_assignments_in_workspace(None, Some(TEST_WORKSPACE)).unwrap()
}

fn list_by_status(status: &str) -> Vec<board_client::Assignment> {
    board_client::list_assignments_in_workspace(Some(status), Some(TEST_WORKSPACE)).unwrap()
}

// ── board_client::is_available ──

#[test]
fn test_board_plugin_is_available() {
    require_board_plugin();
    assert!(board_client::is_available());
}

// ── board_client::list_assignments ──

#[test]
fn test_list_assignments_returns_items() {
    require_board_plugin();
    let assignments = list_all();
    assert!(!assignments.is_empty(), "Expected at least one assignment in workspace '{TEST_WORKSPACE}'");
    for a in &assignments {
        assert!(!a.id.is_empty(), "Assignment missing id");
    }
}

#[test]
fn test_list_assignments_filter_by_status() {
    require_board_plugin();
    let all = list_all();
    let done = list_by_status("done");
    let ready = list_by_status("ready-for-dev");

    assert!(done.len() <= all.len());
    assert!(ready.len() <= all.len());

    for a in &done {
        assert_eq!(a.status, "done", "Expected done, got {}", a.status);
    }
    for a in &ready {
        assert_eq!(a.status, "ready-for-dev", "Expected ready-for-dev, got {}", a.status);
    }
}

#[test]
fn test_list_assignments_filter_nonexistent_status() {
    require_board_plugin();
    let result = list_by_status("nonexistent-status");
    assert!(result.is_empty());
}

// ── board_client::update_assignment ──

#[test]
fn test_update_assignment_status() {
    require_board_plugin();
    // Find a task to update
    let assignments = list_all();
    let task = assignments.first().expect("Need at least one task");
    let original_status = task.status.clone();

    let new_status = if original_status == "review" { "in-progress" } else { "review" };
    board_client::update_assignment(
        &task.id,
        &serde_json::json!({"status": new_status}),
    )
    .unwrap();

    let updated = list_all()
        .into_iter()
        .find(|a| a.id == task.id)
        .expect("Task should still exist");
    assert_eq!(updated.status, new_status);

    // Restore original status
    board_client::update_assignment(
        &task.id,
        &serde_json::json!({"status": original_status}),
    )
    .unwrap();
}

// ── board_client::add_comment ──

#[test]
fn test_add_comment() {
    require_board_plugin();
    let assignments = list_all();
    let task = assignments.first().expect("Need at least one task");

    // Add a comment
    board_client::add_comment(&task.id, "Integration test comment", "test-runner").unwrap();

    // Verify via Pulse task API
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}/api/v1/tasks/{}", port, task.id);
    let body = reqwest::blocking::get(&url).unwrap().text().unwrap();
    let val: serde_json::Value = serde_json::from_str(&body).unwrap();
    let task_val = val.get("task").unwrap_or(&val);
    let meta = task_val.get("metadata").expect("task should have metadata");
    let comments = meta.get("comments").and_then(|c| c.as_array()).expect("should have comments");

    let has_test_comment = comments.iter().any(|c| {
        c.get("content").and_then(|v| v.as_str()) == Some("Integration test comment")
            && c.get("author").and_then(|v| v.as_str()) == Some("test-runner")
    });
    assert!(has_test_comment, "Comment not found. Comments: {:?}", comments);
}

// ── auto_dev integration ──

#[test]
fn test_auto_dev_status_via_board_client() {
    require_board_plugin();
    let config = plugin_coding_pack::workspace::WorkspaceConfig::resolve(None);
    let status = plugin_coding_pack::auto_dev::auto_dev_status(&config).unwrap();

    assert!(status.get("total").is_some(), "Missing 'total' field");
    assert!(status.get("by_status").is_some(), "Missing 'by_status' field");
    // total may be 0 if board_client fetches without workspace filter
    // just check the shape is correct
    assert!(status["total"].is_number());
}

#[test]
fn test_auto_dev_pick_next_task() {
    require_board_plugin();
    let config = plugin_coding_pack::workspace::WorkspaceConfig::resolve(None);
    let task = plugin_coding_pack::auto_dev::pick_next_task(&config).unwrap();
    // Either Some (has ready-for-dev tasks) or None (no ready tasks) — both valid
    if let Some(t) = &task {
        assert!(!t.id.is_empty());
        assert_eq!(t.status, "ready-for-dev");
    }
}

#[test]
fn test_auto_dev_resolve_workflow() {
    use plugin_coding_pack::board_client::Assignment;

    let bug = Assignment {
        labels: vec!["bug".to_string()],
        ..Default::default()
    };
    assert_eq!(plugin_coding_pack::auto_dev::resolve_workflow_id(&bug), "coding-bug-fix");

    let story = Assignment {
        labels: vec!["story".to_string()],
        ..Default::default()
    };
    assert_eq!(plugin_coding_pack::auto_dev::resolve_workflow_id(&story), "coding-story-dev");

    let explicit = Assignment {
        workflow_id: "coding-review".to_string(),
        labels: vec!["bug".to_string()],
        ..Default::default()
    };
    assert_eq!(plugin_coding_pack::auto_dev::resolve_workflow_id(&explicit), "coding-review");
}
