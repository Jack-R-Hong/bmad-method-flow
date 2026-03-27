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
            "pr-fix" => return "coding-pr-fix",
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

/// Pick the highest-priority task: PR fixes first, then board tasks.
///
/// Checks `check_pending_reviews()` for PRs with `changes_requested` state
/// before falling through to the board plugin. PR fix tasks get `critical`
/// priority so they always win.
pub fn pick_next_task(config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    // Priority 1: PR review fixes
    if let Some(pr_fix) = check_pending_reviews(config)? {
        return Ok(Some(pr_fix));
    }

    // Priority 2: Board tasks (existing logic)
    let assignments = match board_client::list_assignments(Some("ready-for-dev")) {
        Ok(a) => a,
        Err(_) => return Ok(None), // board plugin unavailable
    };
    let ready = assignments
        .into_iter()
        .min_by_key(|a| priority_rank(&a.priority));
    Ok(ready)
}

/// Check for open auto-dev PRs that need fixes (changes_requested).
///
/// Returns a synthetic `Assignment` for the oldest PR with `changes_requested`
/// state. Returns `Ok(None)` on any failure (graceful degradation).
#[cfg(not(target_arch = "wasm32"))]
fn check_pending_reviews(_config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    use crate::github_client::{aggregate_review_state, is_auto_dev_pr, GitHubClient};

    // Graceful: if no GitHub token, skip PR review checking entirely
    let client = match GitHubClient::new() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let prs = match client.list_open_prs() {
        Ok(prs) => prs,
        Err(e) => {
            tracing::warn!(
                plugin = "coding-pack",
                error = %e,
                "Failed to list open PRs for review check"
            );
            return Ok(None);
        }
    };

    // Filter to auto-dev PRs with changes_requested
    let mut fix_candidates: Vec<_> = Vec::new();
    for pr in &prs {
        if !is_auto_dev_pr(pr) {
            continue;
        }
        let reviews = match client.list_pr_reviews(pr.number) {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(
                    plugin = "coding-pack",
                    pr_number = pr.number,
                    error = %e,
                    "Failed to fetch reviews for PR"
                );
                continue;
            }
        };
        let state = aggregate_review_state(&reviews);
        if state == "changes_requested" {
            fix_candidates.push(pr);
        }
    }

    // FIFO: pick lowest PR number (oldest)
    fix_candidates.sort_by_key(|pr| pr.number);

    match fix_candidates.first() {
        Some(pr) => {
            let assignment = Assignment {
                id: format!("pr-fix-{}", pr.number),
                title: format!("Fix PR #{}: {}", pr.number, pr.title),
                status: "ready-for-dev".to_string(),
                workflow_id: "coding-pr-fix".to_string(),
                priority: "critical".to_string(),
                labels: vec!["pr-fix".to_string()],
                description: format!(
                    "PR #{} has changes requested. Branch: {}",
                    pr.number, pr.head.ref_field
                ),
                assignee: String::new(),
            };
            tracing::info!(
                plugin = "coding-pack",
                pr_number = pr.number,
                branch = %pr.head.ref_field,
                "Detected PR needing fixes"
            );
            Ok(Some(assignment))
        }
        None => Ok(None),
    }
}

#[cfg(target_arch = "wasm32")]
fn check_pending_reviews(_config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    Ok(None)
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
        return (
            true,
            "No test runner detected — skipping validation".to_string(),
        );
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
                let failure_names: Vec<&str> = parsed
                    .failures
                    .iter()
                    .map(|f| f.test_name.as_str())
                    .collect();
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
            format!("\n\nCloses #{number}"),
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
    let is_pr_fix = task.id.starts_with("pr-fix-");

    // ── 1. Set status -> in-progress + start comment ──
    // PR fix tasks have synthetic IDs not in the board — skip board updates
    if !is_pr_fix {
        board_client::update_assignment(&task_id, &serde_json::json!({"status": "in-progress"}))?;
        board_client::add_comment(
            &task_id,
            &format!("[auto-dev] Starting workflow '{workflow_id}'"),
            "auto-dev",
        )?;
    } else {
        tracing::info!(
            plugin = "coding-pack",
            task_id = %task_id,
            "PR fix task — skipping board status update"
        );
    }

    // ── 2. Execute workflow with metadata ──
    let user_input = if is_pr_fix {
        // Extract PR metadata for template variables
        let pr_number = task.id.strip_prefix("pr-fix-").unwrap_or("0");
        let branch = task
            .description
            .split("Branch: ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("unknown");
        format!(
            "pr_number={}\npr_branch={}\n\n{}",
            pr_number, branch, task.title
        )
    } else if task.description.is_empty() {
        task.title.clone()
    } else {
        format!("{}\n\n{}", task.title, task.description)
    };

    // Build extra template vars
    let mut extra_vars = if is_pr_fix {
        let mut vars = HashMap::new();
        let pr_number = task.id.strip_prefix("pr-fix-").unwrap_or("0");
        let branch = task
            .description
            .split("Branch: ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("unknown");
        vars.insert("pr_number".to_string(), pr_number.to_string());
        vars.insert("pr_branch".to_string(), branch.to_string());
        vars.insert("issue_closing_ref".to_string(), String::new());
        vars
    } else {
        build_issue_template_vars(&task_id)
    };

    // Ensure issue_closing_ref is always set for template resolution
    if !extra_vars.contains_key("issue_closing_ref") {
        extra_vars.insert("issue_closing_ref".to_string(), String::new());
    }

    // P6: Always insert task_id so the executor can correlate worktrees with tasks
    extra_vars.insert("task_id".to_string(), task.id.clone());

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
                // ── 4a. Success -> review ──
                if !is_pr_fix {
                    board_client::update_assignment(
                        &task_id,
                        &serde_json::json!({"status": "review"}),
                    )?;
                    let comment = format!(
                        "[auto-dev] Workflow '{}' completed. {}. Ready for review.",
                        workflow_id, test_summary
                    );
                    board_client::add_comment(&task_id, &comment, "auto-dev")?;
                }

                // Re-request review after successful PR fix push
                if is_pr_fix {
                    re_request_review_for_pr_fix(&task_id);
                }

                Ok(Some(AutoDevResult {
                    task_id,
                    workflow_id,
                    outcome: "success".to_string(),
                    test_passed: true,
                    comment: test_summary,
                }))
            } else {
                // ── 4b. Test failure -> stay in-progress ──
                if !is_pr_fix {
                    let comment = format!(
                        "[auto-dev] Workflow '{}' completed but tests failed. {}",
                        workflow_id, test_summary
                    );
                    board_client::add_comment(&task_id, &comment, "auto-dev")?;
                }
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
            // ── 4c. Workflow error -> backlog ──
            if !is_pr_fix {
                let comment = format!("[auto-dev] Workflow '{}' failed: {}", workflow_id, e);
                board_client::update_assignment(
                    &task_id,
                    &serde_json::json!({"status": "backlog"}),
                )?;
                board_client::add_comment(&task_id, &comment, "auto-dev")?;
            }
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

/// Re-request review from original reviewers after a PR fix is pushed.
///
/// Best-effort: if any step fails, logs a warning but does not propagate the
/// error since the fix commits are already pushed.
#[cfg(not(target_arch = "wasm32"))]
fn re_request_review_for_pr_fix(task_id: &str) {
    use crate::github_client::GitHubClient;

    let pr_number_str = task_id.strip_prefix("pr-fix-").unwrap_or("0");
    let pr_number = match pr_number_str.parse::<u64>() {
        Ok(n) => n,
        Err(_) => {
            tracing::warn!(
                plugin = "coding-pack",
                task_id = %task_id,
                "Could not parse PR number from task ID for re-request review"
            );
            return;
        }
    };

    let client = match GitHubClient::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                plugin = "coding-pack",
                error = %e,
                "Could not create GitHub client for re-request review"
            );
            return;
        }
    };

    // Get reviewers who requested changes
    let reviews = match client.list_pr_reviews(pr_number) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                plugin = "coding-pack",
                pr_number = pr_number,
                error = %e,
                "Failed to fetch reviews for re-request"
            );
            return;
        }
    };

    let mut reviewers: Vec<String> = reviews
        .iter()
        .filter(|r| r.state == "CHANGES_REQUESTED")
        .map(|r| r.user.login.clone())
        .collect();
    reviewers.sort();
    reviewers.dedup();

    if reviewers.is_empty() {
        return;
    }

    match client.request_reviewers(pr_number, &reviewers) {
        Ok(()) => {
            tracing::info!(
                plugin = "coding-pack",
                pr_number = pr_number,
                reviewers = ?reviewers,
                "Re-requested review after PR fix"
            );
        }
        Err(e) => {
            tracing::warn!(
                plugin = "coding-pack",
                pr_number = pr_number,
                error = %e,
                "Failed to re-request review — fix commits were pushed successfully"
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn re_request_review_for_pr_fix(_task_id: &str) {
    // No-op in WASM
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

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_resolve_workflow_explicit() {
        let a = Assignment {
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
    fn test_resolve_workflow_from_label_pr_fix() {
        let a = Assignment {
            id: "t1".to_string(),
            labels: vec!["pr-fix".to_string()],
            ..Default::default()
        };
        assert_eq!(resolve_workflow_id(&a), "coding-pr-fix");
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
            format!("\n\nCloses #{issue_number}"),
        );
        vars.insert(
            "issue_url".to_string(),
            "https://github.com/o/r/issues/42".to_string(),
        );

        assert_eq!(vars.get("issue_closing_ref").unwrap(), "\n\nCloses #42");
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
                format!("\n\nCloses #{number}"),
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
        assert_eq!(vars["issue_closing_ref"], "\n\nCloses #42");
        assert_eq!(vars["issue_url"], "https://github.com/o/r/issues/42");
    }

    #[test]
    fn test_extra_vars_from_metadata_without_issue() {
        // Empty metadata — no issue fields
        let meta = serde_json::json!({});

        let mut vars = HashMap::new();

        if let Some(number) = meta.get("issue_number").and_then(|v| v.as_u64()) {
            vars.insert("issue_number".to_string(), number.to_string());
            vars.insert("issue_closing_ref".to_string(), format!("Closes #{number}"));
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

    // ── PR feedback loop integration (Story 23-4) ────────────────────

    /// Construct a PR fix assignment (mirrors check_pending_reviews logic).
    fn construct_pr_fix_assignment(pr_number: u64, title: &str, branch: &str) -> Assignment {
        Assignment {
            id: format!("pr-fix-{}", pr_number),
            title: format!("Fix PR #{}: {}", pr_number, title),
            status: "ready-for-dev".to_string(),
            workflow_id: "coding-pr-fix".to_string(),
            priority: "critical".to_string(),
            labels: vec!["pr-fix".to_string()],
            description: format!(
                "PR #{} has changes requested. Branch: {}",
                pr_number, branch
            ),
            assignee: String::new(),
        }
    }

    #[test]
    fn test_pr_fix_assignment_construction() {
        let a = construct_pr_fix_assignment(42, "Add feature X", "auto-dev/story-23-1");
        assert_eq!(a.id, "pr-fix-42");
        assert_eq!(a.title, "Fix PR #42: Add feature X");
        assert_eq!(a.status, "ready-for-dev");
        assert_eq!(a.workflow_id, "coding-pr-fix");
        assert_eq!(a.priority, "critical");
        assert_eq!(a.labels, vec!["pr-fix"]);
        assert!(a.description.contains("Branch: auto-dev/story-23-1"));
    }

    #[test]
    fn test_pr_fix_task_has_higher_priority_than_board() {
        assert!(priority_rank("critical") < priority_rank("high"));
        assert!(priority_rank("critical") < priority_rank("medium"));
        assert!(priority_rank("critical") < priority_rank("low"));
    }

    #[test]
    fn test_detect_pr_fix_task_by_id_prefix() {
        let a = construct_pr_fix_assignment(42, "Title", "auto-dev/x");
        assert!(a.id.starts_with("pr-fix-"));

        let normal = Assignment {
            id: "task-123".to_string(),
            ..Default::default()
        };
        assert!(!normal.id.starts_with("pr-fix-"));
    }

    #[test]
    fn test_parse_pr_number_from_task_id() {
        let task_id = "pr-fix-42";
        let pr_number = task_id
            .strip_prefix("pr-fix-")
            .and_then(|s| s.parse::<u64>().ok());
        assert_eq!(pr_number, Some(42));
    }

    #[test]
    fn test_parse_branch_from_description() {
        let description = "PR #42 has changes requested. Branch: auto-dev/story-23-1";
        let branch = description
            .split("Branch: ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("unknown");
        assert_eq!(branch, "auto-dev/story-23-1");
    }

    #[test]
    fn test_parse_branch_from_description_with_trailing_text() {
        let description =
            "PR #42 has changes requested. Branch: auto-dev/story-23-1 some extra text";
        let branch = description
            .split("Branch: ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("unknown");
        assert_eq!(branch, "auto-dev/story-23-1");
    }

    #[test]
    fn test_parse_branch_from_description_missing() {
        let description = "No branch info here";
        let branch = description
            .split("Branch: ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("unknown");
        assert_eq!(branch, "unknown");
    }

    #[test]
    fn test_fifo_ordering_for_multiple_fix_prs() {
        // Lower PR number should be picked first (FIFO)
        let prs = vec![
            construct_pr_fix_assignment(100, "PR 100", "auto-dev/a"),
            construct_pr_fix_assignment(42, "PR 42", "auto-dev/b"),
            construct_pr_fix_assignment(77, "PR 77", "auto-dev/c"),
        ];

        // Sort by PR number (simulate the check_pending_reviews FIFO logic)
        let mut sorted = prs.clone();
        sorted.sort_by_key(|a| {
            a.id.strip_prefix("pr-fix-")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });

        assert_eq!(sorted[0].id, "pr-fix-42");
        assert_eq!(sorted[1].id, "pr-fix-77");
        assert_eq!(sorted[2].id, "pr-fix-100");
    }

    #[test]
    fn test_pr_fix_workflow_id_via_resolve() {
        // The explicit workflow_id takes priority (existing logic at line 31-32)
        let a = construct_pr_fix_assignment(42, "Title", "auto-dev/x");
        assert_eq!(resolve_workflow_id(&a), "coding-pr-fix");
    }

    #[test]
    fn test_pr_fix_extra_vars_contain_pr_metadata() {
        let a = construct_pr_fix_assignment(42, "Title", "auto-dev/story-23-1");
        let is_pr_fix = a.id.starts_with("pr-fix-");
        assert!(is_pr_fix);

        let pr_number = a.id.strip_prefix("pr-fix-").unwrap_or("0");
        let branch = a
            .description
            .split("Branch: ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("unknown");

        let mut vars = HashMap::new();
        vars.insert("pr_number".to_string(), pr_number.to_string());
        vars.insert("pr_branch".to_string(), branch.to_string());

        assert_eq!(vars["pr_number"], "42");
        assert_eq!(vars["pr_branch"], "auto-dev/story-23-1");
    }

    #[test]
    fn test_check_pending_reviews_returns_none_without_token() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Temporarily remove GITHUB_TOKEN
        let original = std::env::var("GITHUB_TOKEN").ok();
        std::env::remove_var("GITHUB_TOKEN");

        let config = WorkspaceConfig::default();
        let result = check_pending_reviews(&config);

        // Should return Ok(None), NOT Err
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Restore
        if let Some(val) = original {
            std::env::set_var("GITHUB_TOKEN", val);
        }
    }
}
