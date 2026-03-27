//! Auto-dev loop — board-driven autonomous workflow execution.
//!
//! Picks `ready-for-dev` tasks from the board, runs the appropriate workflow,
//! validates with tests, and updates the board with results.

use crate::board_store::{self, StoreAssignment};
use crate::executor;
use crate::test_parser;
use crate::workspace::WorkspaceConfig;
use pulse_plugin_sdk::error::WitPluginError;
use serde::Serialize;

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
pub fn resolve_workflow_id(assignment: &StoreAssignment) -> &str {
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

/// Pick the highest-priority `ready-for-dev` assignment from the store.
/// Returns `None` if no tasks are ready.
pub fn pick_next_task(config: &WorkspaceConfig) -> Result<Option<StoreAssignment>, WitPluginError> {
    if !board_store::store_exists(&config.base_dir) {
        return Ok(None);
    }
    let store = board_store::load_store(&config.base_dir)?;
    let ready = store
        .assignments
        .into_iter()
        .filter(|a| a.status == "ready-for-dev")
        .min_by_key(|a| priority_rank(&a.priority));
    Ok(ready)
}

// ── Validation ────────────────────────────────────────────────────────────

/// Detect project type and run tests. Returns (passed, output_summary).
fn run_validation(config: &WorkspaceConfig) -> (bool, String) {
    let base = &config.base_dir;

    // Detect and run test command
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
    let base_dir = &config.base_dir;

    // ── 1. Set status → in-progress + start comment ──
    board_store::update_assignment(
        base_dir,
        &task_id,
        &serde_json::json!({"status": "in-progress"}),
    )?;
    board_store::add_comment(
        base_dir,
        &task_id,
        &serde_json::json!({
            "content": format!("[auto-dev] Starting workflow '{workflow_id}'"),
            "author": "auto-dev"
        }),
    )?;

    // ── 2. Execute workflow ──
    let user_input = if task.description.is_empty() {
        task.title.clone()
    } else {
        format!("{}\n\n{}", task.title, task.description)
    };

    let workflow_result = executor::execute_workflow_with_config(&workflow_id, &user_input, config);

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
                board_store::update_assignment(
                    base_dir,
                    &task_id,
                    &serde_json::json!({"status": "review"}),
                )?;
                let comment = format!(
                    "[auto-dev] Workflow '{}' completed. {}. Ready for review.",
                    workflow_id, test_summary
                );
                board_store::add_comment(
                    base_dir,
                    &task_id,
                    &serde_json::json!({"content": comment, "author": "auto-dev"}),
                )?;
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
                board_store::add_comment(
                    base_dir,
                    &task_id,
                    &serde_json::json!({"content": comment, "author": "auto-dev"}),
                )?;
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
            board_store::update_assignment(
                base_dir,
                &task_id,
                &serde_json::json!({"status": "backlog"}),
            )?;
            board_store::add_comment(
                base_dir,
                &task_id,
                &serde_json::json!({"content": comment, "author": "auto-dev"}),
            )?;
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
pub fn auto_dev_status(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    if !board_store::store_exists(&config.base_dir) {
        return Ok(serde_json::json!({
            "total": 0,
            "by_status": {},
            "next_task": null
        }));
    }
    let store = board_store::load_store(&config.base_dir)?;
    let assignments = &store.assignments;

    let mut by_status = std::collections::BTreeMap::new();
    for a in assignments {
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
    use crate::board_store::{BoardStore, StoreAssignment};

    fn setup_store_with_tasks(base_dir: &std::path::Path, assignments: Vec<StoreAssignment>) {
        let store = BoardStore {
            version: 1,
            project: "test".to_string(),
            last_updated: "2026-03-27".to_string(),
            synced_from: None,
            epics: vec![],
            assignments,
        };
        board_store::save_store(base_dir, &store).unwrap();
    }

    fn make_assignment(id: &str, status: &str, priority: &str, labels: Vec<&str>) -> StoreAssignment {
        StoreAssignment {
            id: id.to_string(),
            title: format!("Task {id}"),
            status: status.to_string(),
            description: format!("Description for {id}"),
            assignee: String::new(),
            priority: priority.to_string(),
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            tasks: vec![],
            comments: vec![],
            workflow_id: String::new(),
        }
    }

    // ── resolve_workflow_id ──

    #[test]
    fn test_resolve_workflow_explicit() {
        let mut a = make_assignment("t1", "ready-for-dev", "high", vec![]);
        a.workflow_id = "coding-feature-dev".to_string();
        assert_eq!(resolve_workflow_id(&a), "coding-feature-dev");
    }

    #[test]
    fn test_resolve_workflow_from_label_story() {
        let a = make_assignment("t1", "ready-for-dev", "high", vec!["story"]);
        assert_eq!(resolve_workflow_id(&a), "coding-story-dev");
    }

    #[test]
    fn test_resolve_workflow_from_label_bug() {
        let a = make_assignment("t1", "ready-for-dev", "high", vec!["bug"]);
        assert_eq!(resolve_workflow_id(&a), "coding-bug-fix");
    }

    #[test]
    fn test_resolve_workflow_from_label_refactor() {
        let a = make_assignment("t1", "ready-for-dev", "high", vec!["refactor"]);
        assert_eq!(resolve_workflow_id(&a), "coding-refactor");
    }

    #[test]
    fn test_resolve_workflow_default() {
        let a = make_assignment("t1", "ready-for-dev", "high", vec!["unrelated"]);
        assert_eq!(resolve_workflow_id(&a), "coding-quick-dev");
    }

    #[test]
    fn test_resolve_workflow_explicit_overrides_label() {
        let mut a = make_assignment("t1", "ready-for-dev", "high", vec!["bug"]);
        a.workflow_id = "coding-story-dev".to_string();
        assert_eq!(resolve_workflow_id(&a), "coding-story-dev");
    }

    // ── pick_next_task ──

    #[test]
    fn test_pick_next_no_store() {
        let dir = tempfile::tempdir().unwrap();
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert!(pick_next_task(&config).unwrap().is_none());
    }

    #[test]
    fn test_pick_next_no_ready_tasks() {
        let dir = tempfile::tempdir().unwrap();
        setup_store_with_tasks(
            dir.path(),
            vec![
                make_assignment("t1", "backlog", "high", vec![]),
                make_assignment("t2", "in-progress", "high", vec![]),
                make_assignment("t3", "done", "high", vec![]),
            ],
        );
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert!(pick_next_task(&config).unwrap().is_none());
    }

    #[test]
    fn test_pick_next_selects_highest_priority() {
        let dir = tempfile::tempdir().unwrap();
        setup_store_with_tasks(
            dir.path(),
            vec![
                make_assignment("low-task", "ready-for-dev", "low", vec![]),
                make_assignment("critical-task", "ready-for-dev", "critical", vec![]),
                make_assignment("medium-task", "ready-for-dev", "medium", vec![]),
            ],
        );
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let task = pick_next_task(&config).unwrap().unwrap();
        assert_eq!(task.id, "critical-task");
    }

    #[test]
    fn test_pick_next_same_priority_picks_first() {
        let dir = tempfile::tempdir().unwrap();
        setup_store_with_tasks(
            dir.path(),
            vec![
                make_assignment("first", "ready-for-dev", "high", vec![]),
                make_assignment("second", "ready-for-dev", "high", vec![]),
            ],
        );
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let task = pick_next_task(&config).unwrap().unwrap();
        assert_eq!(task.id, "first");
    }

    // ── auto_dev_next (board state transitions) ──

    #[test]
    fn test_auto_dev_next_no_tasks_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let result = auto_dev_next(&config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_auto_dev_next_workflow_error_reverts_to_backlog() {
        let dir = tempfile::tempdir().unwrap();
        // Task with explicit non-existent workflow_id → guaranteed workflow error
        let mut task = make_assignment("t1", "ready-for-dev", "high", vec![]);
        task.workflow_id = "nonexistent-workflow".to_string();
        setup_store_with_tasks(dir.path(), vec![task]);
        // Point workflows_dir to temp dir (no workflow YAMLs there)
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            workflows_dir: dir.path().join("config/workflows"),
            ..Default::default()
        };

        let result = auto_dev_next(&config).unwrap().unwrap();
        assert_eq!(result.outcome, "workflow_error");
        assert!(!result.test_passed);

        // Verify board was updated
        let store = board_store::load_store(dir.path()).unwrap();
        let task = &store.assignments[0];
        assert_eq!(task.status, "backlog");
        // Should have 2 comments: start + error
        assert_eq!(task.comments.len(), 2);
        assert!(task.comments[0].content.contains("[auto-dev] Starting"));
        assert!(task.comments[1].content.contains("failed"));
    }

    // ── priority_rank ──

    #[test]
    fn test_priority_ordering() {
        assert!(priority_rank("critical") < priority_rank("high"));
        assert!(priority_rank("high") < priority_rank("medium"));
        assert!(priority_rank("medium") < priority_rank("low"));
        assert!(priority_rank("low") < priority_rank("unknown"));
    }

    // ── auto_dev_status ──

    #[test]
    fn test_auto_dev_status_no_store() {
        let dir = tempfile::tempdir().unwrap();
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let status = auto_dev_status(&config).unwrap();
        assert_eq!(status["total"], 0);
        assert!(status["next_task"].is_null());
    }

    #[test]
    fn test_auto_dev_status_with_tasks() {
        let dir = tempfile::tempdir().unwrap();
        setup_store_with_tasks(
            dir.path(),
            vec![
                make_assignment("t1", "backlog", "low", vec![]),
                make_assignment("t2", "ready-for-dev", "high", vec!["bug"]),
                make_assignment("t3", "ready-for-dev", "critical", vec!["story"]),
                make_assignment("t4", "in-progress", "medium", vec![]),
                make_assignment("t5", "done", "medium", vec![]),
            ],
        );
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let status = auto_dev_status(&config).unwrap();
        assert_eq!(status["total"], 5);
        assert_eq!(status["by_status"]["backlog"], 1);
        assert_eq!(status["by_status"]["ready-for-dev"], 2);
        assert_eq!(status["by_status"]["in-progress"], 1);
        assert_eq!(status["by_status"]["done"], 1);
        // next_task should be the critical one
        assert_eq!(status["next_task"]["id"], "t3");
        assert_eq!(status["next_task"]["workflow"], "coding-story-dev");
    }

    // ── auto_dev_watch ──

    #[test]
    fn test_auto_dev_watch_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let results = auto_dev_watch(&config, Some(5)).unwrap();
        assert!(results.is_empty());
    }
}
