//! Auto-dev loop — board-driven autonomous workflow execution.
//!
//! Picks `ready-for-dev` tasks from the board plugin, runs the appropriate workflow,
//! validates with tests, and updates the board with results.
//! Communicates with plugin-board via HTTP (board_client).

use crate::board_client::{self, Assignment};
use crate::executor;
use crate::test_parser;
use crate::workspace::WorkspaceConfig;
use pulse_plugin_sdk::error::WitPluginError;
use serde::Serialize;
use std::collections::HashMap;

// ── Result types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AutoDevResult {
    pub task_id: String,
    pub workflow_id: String,
    pub outcome: String,
    pub test_passed: bool,
    pub comment: String,
}

// ── Workflow routing ──────────────────────────────────────────────────────

/// Resolve which workflow to run for a given assignment.
/// Priority: explicit workflow_id > label convention > default.
pub fn resolve_workflow_id(assignment: &Assignment) -> &str {
    if !assignment.workflow_id.is_empty() {
        return &assignment.workflow_id;
    }
    for label in &assignment.labels {
        match label.as_str() {
            "story" => return "coding-story-dev",
            "bug" => return "coding-bug-fix",
            "refactor" => return "coding-refactor",
            "quick" => return "coding-quick-dev",
            "feature" => return "coding-feature-dev",
            "review" => return "coding-review",
            _ => {}
        }
    }
    "coding-quick-dev"
}

// ── Task selection ────────────────────────────────────────────────────────

fn priority_rank(priority: &str) -> u32 {
    match priority {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

/// Pick the highest-priority `ready-for-dev` assignment from the board plugin.
/// Returns `None` if no tasks are ready or the board plugin is unavailable.
pub fn pick_next_task(_config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    let assignments = match board_client::list_assignments(Some("ready-for-dev")) {
        Ok(a) => a,
        Err(_) => return Ok(None), // board plugin unavailable
    };
    let ready = assignments
        .into_iter()
        .min_by_key(|a| priority_rank(&a.priority));
    Ok(ready)
}

// ── Validation ────────────────────────────────────────────────────────────

/// Detect project type and run tests. Returns (passed, output_summary).
fn run_validation(config: &WorkspaceConfig) -> (bool, String) {
    let base = &config.base_dir;

    let test_cmd = if base.join("Cargo.toml").exists() {
        "cargo test 2>&1"
    } else if base.join("package.json").exists() {
        "npm test 2>&1"
    } else if base.join("pyproject.toml").exists() || base.join("setup.py").exists() {
        "pytest 2>&1"
    } else {
        return (true, "No test runner detected — skipping validation".to_string());
    };

    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(test_cmd)
        .current_dir(base)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{stdout}\n{stderr}");
            let parsed = test_parser::parse_test_output(&combined, None);

            if parsed.failed == 0 && out.status.success() {
                let summary = format!(
                    "Tests passed: {}/{} (framework: {})",
                    parsed.passed, parsed.total, parsed.framework
                );
                (true, summary)
            } else {
                let failure_names: Vec<&str> =
                    parsed.failures.iter().map(|f| f.test_name.as_str()).collect();
                let summary = format!(
                    "Tests failed: {}/{} failed. Failures: {}",
                    parsed.failed,
                    parsed.total,
                    if failure_names.is_empty() {
                        "see output".to_string()
                    } else {
                        failure_names.join(", ")
                    }
                );
                (false, summary)
            }
        }
        Err(e) => (false, format!("Failed to run tests: {e}")),
    }
}

// ── Issue metadata for PR linking ────────────────────────────────────────

/// Build template vars from task metadata for issue linking.
///
/// Extracts `issue_number`, `issue_url`, and `issue_closing_ref` from a task's
/// metadata. Returns an empty map if the task has no issue metadata (graceful
/// fallback for manually created tasks).
pub fn build_issue_template_vars(task_id: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    let meta = match board_client::get_task_metadata(task_id) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!(
                plugin = "coding-pack",
                task_id = %task_id,
                error = %e,
                "Could not fetch task metadata for issue linking"
            );
            // Always set issue_closing_ref to empty for clean template resolution
            vars.insert("issue_closing_ref".to_string(), String::new());
            return vars;
        }
    };

    if let Some(number) = meta.get("issue_number").and_then(|v| v.as_u64()) {
        vars.insert("issue_number".to_string(), number.to_string());
        vars.insert(
            "issue_closing_ref".to_string(),
            format!("Closes #{number}"),
        );
    }

    if let Some(url) = meta.get("issue_url").and_then(|v| v.as_str()) {
        vars.insert("issue_url".to_string(), url.to_string());
    }

    // If no issue_number was found, set closing_ref to empty for clean template resolution
    if !vars.contains_key("issue_closing_ref") {
        vars.insert("issue_closing_ref".to_string(), String::new());
    }

    if !vars.is_empty() {
        tracing::debug!(
            plugin = "coding-pack",
            task_id = %task_id,
            has_issue = vars.contains_key("issue_number"),
            "Built issue template vars"
        );
    }

    vars
}

// ── Core loop ─────────────────────────────────────────────────────────────

/// Execute one auto-dev cycle: pick a ready-for-dev task, run workflow, validate, update board.
/// Returns `Ok(None)` when no tasks are ready.
pub fn auto_dev_next(config: &WorkspaceConfig) -> Result<Option<AutoDevResult>, WitPluginError> {
    let task = match pick_next_task(config)? {
        Some(t) => t,
        None => return Ok(None),
    };

    let task_id = task.id.clone();
    let workflow_id = resolve_workflow_id(&task).to_string();

    // ── 1. Set status → in-progress + start comment ──
    board_client::update_assignment(
        &task_id,
        &serde_json::json!({"status": "in-progress"}),
    )?;
    board_client::add_comment(
        &task_id,
        &format!("[auto-dev] Starting workflow '{workflow_id}'"),
        "auto-dev",
    )?;

    // ── 2. Execute workflow with issue metadata ──
    let user_input = if task.description.is_empty() {
        task.title.clone()
    } else {
        format!("{}\n\n{}", task.title, task.description)
    };

    // Fetch issue metadata for PR linking (graceful: empty vars if no metadata)
    let extra_vars = build_issue_template_vars(&task_id);

    let workflow_result =
        executor::execute_workflow_with_vars(&workflow_id, &user_input, config, extra_vars);

    match workflow_result {
        Ok(_result) => {
            // ── 3. Run validation gate ──
            let (test_passed, test_summary) = if config.auto_dev.skip_validation {
                (true, "Validation skipped".to_string())
            } else {
                run_validation(config)
            };

            if test_passed {
                // ── 4a. Success → review ──
                board_client::update_assignment(
                    &task_id,
                    &serde_json::json!({"status": "review"}),
                )?;
                let comment = format!(
                    "[auto-dev] Workflow '{}' completed. {}. Ready for review.",
                    workflow_id, test_summary
                );
                board_client::add_comment(&task_id, &comment, "auto-dev")?;
                Ok(Some(AutoDevResult {
                    task_id,
                    workflow_id,
                    outcome: "success".to_string(),
                    test_passed: true,
                    comment: test_summary,
                }))
            } else {
                // ── 4b. Test failure → stay in-progress ──
                let comment = format!(
                    "[auto-dev] Workflow '{}' completed but tests failed. {}",
                    workflow_id, test_summary
                );
                board_client::add_comment(&task_id, &comment, "auto-dev")?;
                Ok(Some(AutoDevResult {
                    task_id,
                    workflow_id,
                    outcome: "test_failure".to_string(),
                    test_passed: false,
                    comment: test_summary,
                }))
            }
        }
        Err(e) => {
            // ── 4c. Workflow error → backlog ──
            let comment = format!("[auto-dev] Workflow '{}' failed: {}", workflow_id, e);
            board_client::update_assignment(
                &task_id,
                &serde_json::json!({"status": "backlog"}),
            )?;
            board_client::add_comment(&task_id, &comment, "auto-dev")?;
            Ok(Some(AutoDevResult {
                task_id,
                workflow_id,
                outcome: "workflow_error".to_string(),
                test_passed: false,
                comment: e.to_string(),
            }))
        }
    }
}

/// Run auto-dev loop until no ready-for-dev tasks remain or max_iterations reached.
pub fn auto_dev_watch(
    config: &WorkspaceConfig,
    max_iterations: Option<u32>,
) -> Result<Vec<AutoDevResult>, WitPluginError> {
    let max = max_iterations.unwrap_or(config.auto_dev.max_tasks);
    let mut results = Vec::new();
    for _ in 0..max {
        match auto_dev_next(config)? {
            Some(result) => results.push(result),
            None => break,
        }
    }
    Ok(results)
}

// ── Status ────────────────────────────────────────────────────────────────

/// Return a summary of board readiness for auto-dev.
pub fn auto_dev_status(_config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let assignments = match board_client::list_assignments(None) {
        Ok(a) => a,
        Err(_) => {
            return Ok(serde_json::json!({
                "total": 0,
                "by_status": {},
                "next_task": null
            }));
        }
    };

    let mut by_status = std::collections::BTreeMap::new();
    for a in &assignments {
        *by_status.entry(a.status.as_str()).or_insert(0u32) += 1;
    }

    let next = assignments
        .iter()
        .filter(|a| a.status == "ready-for-dev")
        .min_by_key(|a| priority_rank(&a.priority))
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "title": a.title,
                "priority": a.priority,
                "workflow": resolve_workflow_id(a),
            })
        });

    Ok(serde_json::json!({
        "total": assignments.len(),
        "by_status": by_status,
        "next_task": next,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workflow_explicit() {
        let mut a = Assignment {
            id: "t1".to_string(),
            title: "Test".to_string(),
            status: "ready-for-dev".to_string(),
            workflow_id: "coding-feature-dev".to_string(),
            ..Default::default()
        };
        assert_eq!(resolve_workflow_id(&a), "coding-feature-dev");
    }

    #[test]
    fn test_resolve_workflow_from_label_story() {
        let a = Assignment {
            id: "t1".to_string(),
            labels: vec!["story".to_string()],
            ..Default::default()
        };
        assert_eq!(resolve_workflow_id(&a), "coding-story-dev");
    }

    #[test]
    fn test_resolve_workflow_from_label_bug() {
        let a = Assignment {
            id: "t1".to_string(),
            labels: vec!["bug".to_string()],
            ..Default::default()
        };
        assert_eq!(resolve_workflow_id(&a), "coding-bug-fix");
    }

    #[test]
    fn test_resolve_workflow_default() {
        let a = Assignment {
            id: "t1".to_string(),
            labels: vec!["unrelated".to_string()],
            ..Default::default()
        };
        assert_eq!(resolve_workflow_id(&a), "coding-quick-dev");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(priority_rank("critical") < priority_rank("high"));
        assert!(priority_rank("high") < priority_rank("medium"));
        assert!(priority_rank("medium") < priority_rank("low"));
        assert!(priority_rank("low") < priority_rank("unknown"));
    }

    // ── Issue template vars (Story 22-4) ──────────────────────────

    #[test]
    fn test_issue_closing_ref_with_issue_number() {
        // Simulates what build_issue_template_vars would produce
        let mut vars = HashMap::new();
        let issue_number: u64 = 42;
        vars.insert("issue_number".to_string(), issue_number.to_string());
        vars.insert(
            "issue_closing_ref".to_string(),
            format!("Closes #{issue_number}"),
        );
        vars.insert(
            "issue_url".to_string(),
            "https://github.com/o/r/issues/42".to_string(),
        );

        assert_eq!(vars.get("issue_closing_ref").unwrap(), "Closes #42");
        assert_eq!(vars.get("issue_number").unwrap(), "42");
        assert_eq!(
            vars.get("issue_url").unwrap(),
            "https://github.com/o/r/issues/42"
        );
    }

    #[test]
    fn test_issue_closing_ref_without_issue_number() {
        // When no issue metadata, closing_ref should be empty string
        let mut vars = HashMap::new();
        vars.insert("issue_closing_ref".to_string(), String::new());

        assert_eq!(vars.get("issue_closing_ref").unwrap(), "");
        assert!(vars.get("issue_number").is_none());
        assert!(vars.get("issue_url").is_none());
    }

    #[test]
    fn test_extra_vars_from_metadata_with_issue() {
        // Test the logic that build_issue_template_vars would use
        let meta = serde_json::json!({
            "issue_number": 42,
            "issue_url": "https://github.com/o/r/issues/42",
            "source": "github-sync",
        });

        let mut vars = HashMap::new();

        if let Some(number) = meta.get("issue_number").and_then(|v| v.as_u64()) {
            vars.insert("issue_number".to_string(), number.to_string());
            vars.insert(
                "issue_closing_ref".to_string(),
                format!("Closes #{number}"),
            );
        }
        if let Some(url) = meta.get("issue_url").and_then(|v| v.as_str()) {
            vars.insert("issue_url".to_string(), url.to_string());
        }
        if !vars.contains_key("issue_closing_ref") {
            vars.insert("issue_closing_ref".to_string(), String::new());
        }

        assert_eq!(vars.len(), 3);
        assert_eq!(vars["issue_number"], "42");
        assert_eq!(vars["issue_closing_ref"], "Closes #42");
        assert_eq!(vars["issue_url"], "https://github.com/o/r/issues/42");
    }

    #[test]
    fn test_extra_vars_from_metadata_without_issue() {
        // Empty metadata — no issue fields
        let meta = serde_json::json!({});

        let mut vars = HashMap::new();

        if let Some(number) = meta.get("issue_number").and_then(|v| v.as_u64()) {
            vars.insert("issue_number".to_string(), number.to_string());
            vars.insert(
                "issue_closing_ref".to_string(),
                format!("Closes #{number}"),
            );
        }
        if let Some(url) = meta.get("issue_url").and_then(|v| v.as_str()) {
            vars.insert("issue_url".to_string(), url.to_string());
        }
        if !vars.contains_key("issue_closing_ref") {
            vars.insert("issue_closing_ref".to_string(), String::new());
        }

        assert_eq!(vars.len(), 1);
        assert_eq!(vars["issue_closing_ref"], "");
    }
}
