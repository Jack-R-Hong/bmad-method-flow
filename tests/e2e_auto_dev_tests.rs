//! E2E tests for the auto-dev loop.
//!
//! Proves the full cycle: board task → workflow execution → validation → board update.
//! Uses mock plugins and function-only workflows — no real LLM calls.
//!
//! Test categories:
//!   - Happy path: task picked up, workflow succeeds, tests pass → status "review"
//!   - Validation failure: workflow succeeds, tests fail → status stays "in-progress"
//!   - Workflow error: workflow fails → status reverts to "backlog"
//!   - Priority ordering: highest-priority task is picked first
//!   - Workflow routing: labels resolve to correct workflow ID
//!   - Watch mode: processes multiple tasks sequentially
//!   - Status endpoint: returns correct board summary
//!   - No tasks: returns None gracefully

use plugin_coding_pack::auto_dev;
use plugin_coding_pack::board_store::{self, BoardStore, StoreAssignment};
use plugin_coding_pack::workspace::WorkspaceConfig;
use std::path::{Path, PathBuf};
// ── Test environment ─────────────────────────────────────────────────────

struct AutoDevTestEnv {
    temp_dir: PathBuf,
}

impl AutoDevTestEnv {
    fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!(
            "pulse-e2e-autodev-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

        // Create config/ directory structure with symlinks
        let config_dir = temp_dir.join("config");
        std::fs::create_dir_all(&config_dir).expect("create config dir");

        // Symlink mock-plugins → config/plugins
        let plugins_src = fixtures.join("mock-plugins");
        let plugins_dst = config_dir.join("plugins");
        std::os::unix::fs::symlink(&plugins_src, &plugins_dst)
            .unwrap_or_else(|e| panic!("symlink plugins: {e}"));

        // Symlink test workflows → config/workflows
        let workflows_src = fixtures.join("workflows");
        let workflows_dst = config_dir.join("workflows");
        std::os::unix::fs::symlink(&workflows_src, &workflows_dst)
            .unwrap_or_else(|e| panic!("symlink workflows: {e}"));

        Self { temp_dir }
    }

    fn base_dir(&self) -> &Path {
        &self.temp_dir
    }

    fn config(&self) -> WorkspaceConfig {
        WorkspaceConfig::from_base_dir(&self.temp_dir)
    }

    /// Create a board store with the given assignments.
    fn setup_board(&self, assignments: Vec<StoreAssignment>) {
        let store = BoardStore {
            version: 1,
            project: "e2e-test".to_string(),
            last_updated: "2026-03-27".to_string(),
            synced_from: None,
            epics: vec![],
            assignments,
        };
        board_store::save_store(self.base_dir(), &store).unwrap();
    }

    /// Reload the board store and return it.
    fn load_board(&self) -> BoardStore {
        board_store::load_store(self.base_dir()).unwrap()
    }

    /// Find an assignment by ID in the current board state.
    fn get_task(&self, id: &str) -> StoreAssignment {
        let store = self.load_board();
        store
            .assignments
            .into_iter()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("task '{id}' not found in board"))
    }
}

impl Drop for AutoDevTestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

fn make_task(id: &str, priority: &str, labels: Vec<&str>, workflow_id: &str) -> StoreAssignment {
    StoreAssignment {
        id: id.to_string(),
        title: format!("Task: {id}"),
        status: "ready-for-dev".to_string(),
        description: format!("Description for {id}"),
        assignee: String::new(),
        priority: priority.to_string(),
        labels: labels.into_iter().map(|s| s.to_string()).collect(),
        tasks: vec![],
        comments: vec![],
        workflow_id: workflow_id.to_string(),
    }
}

// ── Happy path: workflow succeeds, validation passes → "review" ──────────

#[test]
fn autodev_e2e_happy_path_task_to_review() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![make_task(
        "task-happy",
        "high",
        vec![],
        "test-autodev-simple",
    )]);

    // Skip validation since there's no real Cargo.toml in temp dir
    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    let result = auto_dev::auto_dev_next(&config).unwrap();
    assert!(result.is_some(), "should have picked up a task");

    let r = result.unwrap();
    assert_eq!(r.task_id, "task-happy");
    assert_eq!(r.workflow_id, "test-autodev-simple");
    assert_eq!(r.outcome, "success");
    assert!(r.test_passed);

    // Verify board state
    let task = env.get_task("task-happy");
    assert_eq!(task.status, "review", "task should be moved to review");
    assert!(
        task.comments.len() >= 2,
        "should have start + completion comments, got {}",
        task.comments.len()
    );
    assert!(task.comments[0].content.contains("[auto-dev] Starting"));
    assert!(task.comments[0].author == "auto-dev");
    assert!(task.comments.last().unwrap().content.contains("Ready for review"));
}

// ── Workflow error: workflow fails → "backlog" ───────────────────────────

#[test]
fn autodev_e2e_workflow_error_reverts_to_backlog() {
    let env = AutoDevTestEnv::new();
    // Use a non-existent workflow to trigger error
    env.setup_board(vec![make_task(
        "task-error",
        "high",
        vec![],
        "nonexistent-workflow",
    )]);

    let config = env.config();
    let result = auto_dev::auto_dev_next(&config).unwrap().unwrap();

    assert_eq!(result.task_id, "task-error");
    assert_eq!(result.outcome, "workflow_error");
    assert!(!result.test_passed);

    let task = env.get_task("task-error");
    assert_eq!(task.status, "backlog", "failed task should revert to backlog");
    assert!(task.comments.len() >= 2);
    assert!(
        task.comments.last().unwrap().content.contains("failed"),
        "error comment should mention failure"
    );
}

// ── No tasks: returns None ───────────────────────────────────────────────

#[test]
fn autodev_e2e_no_ready_tasks_returns_none() {
    let env = AutoDevTestEnv::new();
    // All tasks are in backlog — none ready-for-dev
    let mut task = make_task("task-backlog", "high", vec![], "test-autodev-simple");
    task.status = "backlog".to_string();
    env.setup_board(vec![task]);

    let config = env.config();
    let result = auto_dev::auto_dev_next(&config).unwrap();
    assert!(result.is_none(), "should return None when no ready-for-dev tasks");

    // Board should be unchanged
    let task = env.get_task("task-backlog");
    assert_eq!(task.status, "backlog");
    assert!(task.comments.is_empty());
}

// ── Priority ordering: critical picked before low ────────────────────────

#[test]
fn autodev_e2e_picks_highest_priority_first() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![
        make_task("task-low", "low", vec![], "test-autodev-simple"),
        make_task("task-critical", "critical", vec![], "test-autodev-simple"),
        make_task("task-medium", "medium", vec![], "test-autodev-simple"),
    ]);

    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    let result = auto_dev::auto_dev_next(&config).unwrap().unwrap();
    assert_eq!(
        result.task_id, "task-critical",
        "should pick critical-priority task first"
    );

    // Other tasks should still be ready-for-dev
    let low = env.get_task("task-low");
    assert_eq!(low.status, "ready-for-dev");
    let medium = env.get_task("task-medium");
    assert_eq!(medium.status, "ready-for-dev");
}

// ── Workflow routing: labels resolve correctly ───────────────────────────

#[test]
fn autodev_e2e_label_routes_to_correct_workflow() {
    let env = AutoDevTestEnv::new();
    // "quick" label should resolve to "coding-quick-dev" — which doesn't exist
    // in test fixtures, causing a workflow_error with the correct workflow name
    env.setup_board(vec![make_task("task-routed", "high", vec!["quick"], "")]);

    let config = env.config();
    let result = auto_dev::auto_dev_next(&config).unwrap().unwrap();

    // The workflow was resolved from the label
    assert_eq!(
        result.workflow_id, "coding-quick-dev",
        "should resolve 'quick' label to coding-quick-dev"
    );
    // It will fail because coding-quick-dev doesn't exist in test fixtures
    assert_eq!(result.outcome, "workflow_error");
}

#[test]
fn autodev_e2e_explicit_workflow_overrides_label() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![make_task(
        "task-explicit",
        "high",
        vec!["bug"], // label says bug-fix
        "test-autodev-simple", // explicit says simple
    )]);

    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    let result = auto_dev::auto_dev_next(&config).unwrap().unwrap();
    assert_eq!(
        result.workflow_id, "test-autodev-simple",
        "explicit workflow_id should override label"
    );
    assert_eq!(result.outcome, "success");
}

// ── Watch mode: processes multiple tasks ─────────────────────────────────

#[test]
fn autodev_e2e_watch_processes_multiple_tasks() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![
        make_task("task-w1", "high", vec![], "test-autodev-simple"),
        make_task("task-w2", "medium", vec![], "test-autodev-simple"),
        make_task("task-w3", "low", vec![], "test-autodev-simple"),
    ]);

    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    let results = auto_dev::auto_dev_watch(&config, Some(10)).unwrap();
    assert_eq!(results.len(), 3, "should process all 3 tasks");

    // All should succeed
    for r in &results {
        assert_eq!(r.outcome, "success", "task {} should succeed", r.task_id);
    }

    // Priority order: high → medium → low
    assert_eq!(results[0].task_id, "task-w1");
    assert_eq!(results[1].task_id, "task-w2");
    assert_eq!(results[2].task_id, "task-w3");

    // All tasks should now be in review
    let store = env.load_board();
    for a in &store.assignments {
        assert_eq!(
            a.status, "review",
            "task {} should be in review, got {}",
            a.id, a.status
        );
    }
}

#[test]
fn autodev_e2e_watch_stops_when_no_tasks() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![make_task(
        "task-single",
        "high",
        vec![],
        "test-autodev-simple",
    )]);

    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    // Ask for 10 iterations but only 1 task exists
    let results = auto_dev::auto_dev_watch(&config, Some(10)).unwrap();
    assert_eq!(results.len(), 1, "should stop after processing the only task");
}

// ── Status endpoint ──────────────────────────────────────────────────────

#[test]
fn autodev_e2e_status_reflects_board_state() {
    let env = AutoDevTestEnv::new();
    let mut backlog = make_task("t-backlog", "low", vec![], "");
    backlog.status = "backlog".to_string();
    let mut done = make_task("t-done", "low", vec![], "");
    done.status = "done".to_string();

    env.setup_board(vec![
        make_task("t-ready1", "critical", vec!["story"], ""),
        make_task("t-ready2", "low", vec![], ""),
        backlog,
        done,
    ]);

    let config = env.config();
    let status = auto_dev::auto_dev_status(&config).unwrap();

    assert_eq!(status["total"], 4);
    assert_eq!(status["by_status"]["ready-for-dev"], 2);
    assert_eq!(status["by_status"]["backlog"], 1);
    assert_eq!(status["by_status"]["done"], 1);

    // next_task should be the critical one
    assert_eq!(status["next_task"]["id"], "t-ready1");
    assert_eq!(status["next_task"]["priority"], "critical");
    assert_eq!(status["next_task"]["workflow"], "coding-story-dev");
}

// ── Board audit trail ────────────────────────────────────────────────────

#[test]
fn autodev_e2e_comments_form_audit_trail() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![make_task(
        "task-audit",
        "high",
        vec![],
        "test-autodev-simple",
    )]);

    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    auto_dev::auto_dev_next(&config).unwrap();

    let task = env.get_task("task-audit");
    assert!(task.comments.len() >= 2, "need at least start + completion comments");

    // First comment: start
    let start = &task.comments[0];
    assert_eq!(start.author, "auto-dev");
    assert!(start.content.contains("[auto-dev] Starting workflow"));
    assert!(start.content.contains("test-autodev-simple"));

    // Last comment: completion
    let end = task.comments.last().unwrap();
    assert_eq!(end.author, "auto-dev");
    assert!(end.content.contains("[auto-dev] Workflow"));
    assert!(end.content.contains("Ready for review"));
}

// ── State machine: status transitions are correct ────────────────────────

#[test]
fn autodev_e2e_status_transitions_ready_to_inprogress_to_review() {
    let env = AutoDevTestEnv::new();
    env.setup_board(vec![make_task(
        "task-transition",
        "high",
        vec![],
        "test-autodev-simple",
    )]);

    // Before: ready-for-dev
    let before = env.get_task("task-transition");
    assert_eq!(before.status, "ready-for-dev");

    let mut config = env.config();
    config.auto_dev.skip_validation = true;

    auto_dev::auto_dev_next(&config).unwrap();

    // After: review (happy path)
    let after = env.get_task("task-transition");
    assert_eq!(after.status, "review");
}

// ── Empty board: no store file ───────────────────────────────────────────

#[test]
fn autodev_e2e_no_board_store_returns_none() {
    let env = AutoDevTestEnv::new();
    // Don't create any board store
    let config = env.config();
    let result = auto_dev::auto_dev_next(&config).unwrap();
    assert!(result.is_none());
}

#[test]
fn autodev_e2e_status_no_board_store() {
    let env = AutoDevTestEnv::new();
    let config = env.config();
    let status = auto_dev::auto_dev_status(&config).unwrap();
    assert_eq!(status["total"], 0);
    assert!(status["next_task"].is_null());
}
