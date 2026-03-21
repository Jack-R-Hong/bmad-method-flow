//! E2E integration tests for the workflow executor.
//!
//! These tests use mock plugin scripts (bash) and test workflow YAML files
//! to exercise the full execution pipeline without requiring real LLM APIs.
//!
//! Test categories:
//!   - Function-only workflows (no external plugins)
//!   - Agent workflows with mock plugins (JSON-RPC protocol)
//!   - Quality gate pass/block
//!   - Retry loops with test failure feedback
//!   - Template variable propagation
//!   - Optional/required step failure handling
//!   - Timeout handling
//!   - Context assembly between steps
//!   - Dependency chain ordering (DAG)
//!   - Missing plugin error handling

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Mutex to serialize tests that modify environment variables.
/// Prevents race conditions when tests run in parallel.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Resolve the test fixtures base directory.
/// The executor uses `base_dir/config/plugins` and `base_dir/config/workflows`,
/// so we set up a temp directory with symlinks pointing to our fixtures.
struct TestEnv {
    temp_dir: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!(
            "pulse-e2e-exec-{}",
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
        std::os::unix::fs::symlink(&plugins_src, &plugins_dst).unwrap_or_else(|e| {
            panic!(
                "symlink {:?} → {:?}: {}",
                plugins_src, plugins_dst, e
            )
        });

        // Symlink test workflows → config/workflows
        let workflows_src = fixtures.join("workflows");
        let workflows_dst = config_dir.join("workflows");
        std::os::unix::fs::symlink(&workflows_src, &workflows_dst).unwrap_or_else(|e| {
            panic!(
                "symlink {:?} → {:?}: {}",
                workflows_src, workflows_dst, e
            )
        });

        Self { temp_dir }
    }

    fn base_dir(&self) -> &Path {
        &self.temp_dir
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

// ── Helper ──────────────────────────────────────────────────────────────────

fn assert_step_status(result: &serde_json::Value, step_id: &str, expected: &str) {
    let steps = result["steps"]
        .as_array()
        .expect("result should have steps array");
    let step = steps
        .iter()
        .find(|s| s["step_id"].as_str() == Some(step_id))
        .unwrap_or_else(|| panic!("step '{}' not found in result: {:?}", step_id, steps));
    let actual = step["status"].as_str().expect("step should have status");
    assert_eq!(
        actual, expected,
        "step '{}': expected status '{}', got '{}'. Error: {:?}",
        step_id,
        expected,
        actual,
        step.get("error")
    );
}

fn get_step_content_preview(result: &serde_json::Value, step_id: &str) -> Option<String> {
    result["steps"]
        .as_array()?
        .iter()
        .find(|s| s["step_id"].as_str() == Some(step_id))?
        .get("content_preview")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Function-only workflows
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn function_only_linear_pipeline() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-function-only",
        "hello world",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_eq!(result["steps_completed"].as_u64(), Some(3));
    assert_eq!(result["steps_total"].as_u64(), Some(3));

    assert_step_status(&result, "step_a", "success");
    assert_step_status(&result, "step_b", "success");
    assert_step_status(&result, "step_c", "success");
}

#[test]
fn function_only_template_substitution() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-function-only",
        "my-feature",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    // step_a echoes "step_a output for {{input}}" which should be resolved
    let preview = get_step_content_preview(&result, "step_a")
        .expect("step_a should have content");
    assert!(
        preview.contains("my-feature"),
        "template {{{{input}}}} should be substituted in step_a output, got: {}",
        preview
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. DAG ordering — parallel branches (diamond)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parallel_diamond_dag_completes() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-parallel-functions",
        "dag test",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_eq!(result["steps_completed"].as_u64(), Some(4));

    assert_step_status(&result, "root", "success");
    assert_step_status(&result, "branch_a", "success");
    assert_step_status(&result, "branch_b", "success");
    assert_step_status(&result, "join", "success");

    // Verify execution order: root must be first, join must be last
    let steps = result["steps"].as_array().unwrap();
    let root_idx = steps
        .iter()
        .position(|s| s["step_id"].as_str() == Some("root"))
        .unwrap();
    let join_idx = steps
        .iter()
        .position(|s| s["step_id"].as_str() == Some("join"))
        .unwrap();
    assert!(root_idx < join_idx, "root must execute before join");
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Template variable propagation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn template_vars_substituted_in_commands() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-template-vars",
        "add-login-feature",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    assert_eq!(result["status"].as_str(), Some("completed"));

    let preview = get_step_content_preview(&result, "use_input")
        .expect("use_input should have content");
    assert!(
        preview.contains("add-login-feature"),
        "{{{{input}}}} not substituted: {}",
        preview
    );

    let preview2 = get_step_content_preview(&result, "use_multi")
        .expect("use_multi should have content");
    assert!(
        preview2.contains("add-login-feature"),
        "{{{{input}}}} not substituted in second step: {}",
        preview2
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Optional step failure — workflow continues
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn optional_step_failure_does_not_block() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-optional-skip",
        "test optional",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    // Overall status should not be "failed" since the failing step is optional
    let status = result["status"].as_str().unwrap();
    assert_ne!(status, "failed", "optional failure should not cause overall failure");

    assert_step_status(&result, "step_ok", "success");
    assert_step_status(&result, "step_optional_fail", "failed");
    assert_step_status(&result, "step_after_optional", "success");
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Required step failure — downstream skipped
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn required_step_failure_skips_downstream() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-required-fail",
        "test fail",
        env.base_dir(),
    )
    .expect("workflow should return result even on failure");

    assert_eq!(result["status"].as_str(), Some("failed"));

    assert_step_status(&result, "step_ok", "success");
    assert_step_status(&result, "step_fail", "failed");
    assert_step_status(&result, "step_never_runs", "skipped");
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Timeout handling
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn step_timeout_kills_process() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-timeout",
        "timeout test",
        env.base_dir(),
    )
    .expect("workflow should return result on timeout");

    assert_eq!(result["status"].as_str(), Some("failed"));
    assert_step_status(&result, "slow_step", "timeout");

    // Verify the error message mentions the timeout
    let steps = result["steps"].as_array().unwrap();
    let step = steps
        .iter()
        .find(|s| s["step_id"].as_str() == Some("slow_step"))
        .unwrap();
    let error = step["error"].as_str().unwrap_or("");
    assert!(
        error.contains("timed out"),
        "error should mention timeout: {}",
        error
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Missing required plugin
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn missing_required_plugin_returns_error() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-missing-plugin",
        "test",
        env.base_dir(),
    );

    assert!(result.is_err(), "should fail when required plugin missing");
    let err = result.unwrap_err();
    assert!(
        err.message.contains("nonexistent-plugin"),
        "error should name missing plugin: {}",
        err.message
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Nonexistent workflow
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn nonexistent_workflow_returns_error() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "does-not-exist",
        "test",
        env.base_dir(),
    );

    assert!(result.is_err(), "should fail for nonexistent workflow");
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Agent step with mock plugins (bmad-method two-stage)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn agent_step_bmad_method_two_stage() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // Ensure default mock behavior (no leftover env from other tests)
    std::env::set_var("MOCK_CLAUDE_RESPONSE", "default");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-agent-mock",
        "build a login page",
        env.base_dir(),
    )
    .expect("agent workflow should succeed with mocks");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_eq!(result["steps_completed"].as_u64(), Some(2));

    assert_step_status(&result, "architect", "success");
    assert_step_status(&result, "implement", "success");
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Quality gate — approve passes through
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn quality_gate_approve_allows_downstream() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_CLAUDE_RESPONSE", "approve");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-quality-gate",
        "review feature",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_step_status(&result, "implement", "success");
    assert_step_status(&result, "qa_review", "success");
    assert_step_status(&result, "git_commit", "success");
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. Quality gate — reject blocks downstream
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn quality_gate_reject_blocks_downstream() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_CLAUDE_RESPONSE", "reject");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-quality-gate",
        "review feature",
        env.base_dir(),
    )
    .expect("workflow should return result");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(
        result["status"].as_str(),
        Some("failed"),
        "workflow should fail when QA rejects"
    );
    assert_step_status(&result, "implement", "success");
    assert_step_status(&result, "qa_review", "failed");
    assert_step_status(&result, "git_commit", "skipped");

    // Verify error mentions the verdict
    let steps = result["steps"].as_array().unwrap();
    let qa = steps
        .iter()
        .find(|s| s["step_id"].as_str() == Some("qa_review"))
        .unwrap();
    let error = qa["error"].as_str().unwrap_or("");
    assert!(
        error.contains("reject"),
        "qa_review error should mention reject verdict: {}",
        error
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. Retry loop — tests fail, retry triggers
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn retry_loop_retries_on_test_failure() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_TEST_RESULT", "fail");
    std::env::set_var("MOCK_CLAUDE_RESPONSE", "default");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-retry-loop",
        "implement multiply",
        env.base_dir(),
    )
    .expect("workflow should return result even after retry exhaustion");

    std::env::remove_var("MOCK_TEST_RESULT");
    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    // Tests always fail, so after 3 attempts the workflow should fail
    assert_eq!(
        result["status"].as_str(),
        Some("failed"),
        "workflow should fail after retry exhaustion"
    );

    // run_tests should show the final failure with retry metadata
    let steps = result["steps"].as_array().unwrap();
    let test_step = steps
        .iter()
        .find(|s| s["step_id"].as_str() == Some("run_tests"))
        .unwrap();
    let error = test_step["error"].as_str().unwrap_or("");
    assert!(
        error.contains("retry_limit_reached") || error.contains("exit code"),
        "should indicate retry exhaustion or test failure: {}",
        error
    );
}

#[test]
fn retry_loop_succeeds_when_tests_pass() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_TEST_RESULT", "pass");
    std::env::set_var("MOCK_CLAUDE_RESPONSE", "default");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-retry-loop",
        "implement multiply",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    std::env::remove_var("MOCK_TEST_RESULT");
    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_step_status(&result, "dev_implement", "success");
    assert_step_status(&result, "run_tests", "success");
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. Context flow between steps
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn context_flow_parallel_producers_to_consumer() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-context-flow",
        "context test",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_eq!(result["steps_completed"].as_u64(), Some(3));

    assert_step_status(&result, "producer_a", "success");
    assert_step_status(&result, "producer_b", "success");
    assert_step_status(&result, "consumer", "success");
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. Workflow result structure validation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn result_contains_expected_fields() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-function-only",
        "structure test",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    // Verify top-level fields
    assert!(result.get("workflow_id").is_some(), "missing workflow_id");
    assert!(result.get("status").is_some(), "missing status");
    assert!(result.get("steps_completed").is_some(), "missing steps_completed");
    assert!(result.get("steps_total").is_some(), "missing steps_total");
    assert!(result.get("steps").is_some(), "missing steps");
    assert!(
        result.get("total_execution_time_ms").is_some(),
        "missing total_execution_time_ms"
    );

    assert_eq!(result["workflow_id"].as_str(), Some("test-function-only"));

    // Verify step-level fields
    let steps = result["steps"].as_array().unwrap();
    for step in steps {
        assert!(step.get("step_id").is_some(), "step missing step_id");
        assert!(step.get("status").is_some(), "step missing status");
        assert!(
            step.get("execution_time_ms").is_some(),
            "step missing execution_time_ms"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. Execution timing is tracked
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn execution_time_is_recorded() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-function-only",
        "timing test",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    let total_ms = result["total_execution_time_ms"].as_u64().unwrap();
    assert!(total_ms > 0, "total execution time should be > 0");

    let steps = result["steps"].as_array().unwrap();
    for step in steps {
        let step_ms = step["execution_time_ms"].as_u64().unwrap();
        // Each step should have some execution time (even if very fast)
        assert!(step_ms < 30_000, "step took unreasonably long: {}ms", step_ms);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. Branch name extraction from agent output
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn branch_name_extracted_from_agent_output() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_CLAUDE_RESPONSE", "branch");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-agent-mock",
        "add feature xyz",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_step_status(&result, "architect", "success");
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. Agent step failure propagation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn agent_step_failure_propagates() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_CLAUDE_RESPONSE", "fail");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-agent-mock",
        "fail test",
        env.base_dir(),
    )
    .expect("workflow should return result even on agent failure");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("failed"));
    assert_step_status(&result, "architect", "failed");
    assert_step_status(&result, "implement", "skipped");
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. Quality gate — request-changes blocks downstream
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn quality_gate_request_changes_blocks_downstream() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    std::env::set_var("MOCK_CLAUDE_RESPONSE", "request-changes");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-quality-gate",
        "review feature",
        env.base_dir(),
    )
    .expect("workflow should return result");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("failed"));
    assert_step_status(&result, "qa_review", "failed");
    assert_step_status(&result, "git_commit", "skipped");
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. Function step command resolution — command[0] wins over executor
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn function_step_resolves_command_not_executor() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-command-resolution",
        "test",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_step_status(&result, "echo_step", "success");

    // The output should contain the echo text, proving echo was used (not bmad-method binary)
    let preview = get_step_content_preview(&result, "echo_step")
        .expect("echo_step should have content");
    assert!(
        preview.contains("command-resolution-works"),
        "should have used echo command, not bmad-method binary: {}",
        preview
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. Working directory — function step runs in specified directory
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn working_dir_applied_to_function_step() {
    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-working-dir",
        "test",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_step_status(&result, "run_in_dir", "success");

    // pwd should output /tmp (the configured working_dir)
    let preview = get_step_content_preview(&result, "run_in_dir")
        .expect("run_in_dir should have content");
    assert!(
        preview.contains("/tmp"),
        "working_dir should change cwd to /tmp, got: {}",
        preview
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. PR field extraction from agent output
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pr_fields_extracted_from_agent_output() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // Mock claude returns content that looks like a PR (first line = title)
    std::env::set_var("MOCK_CLAUDE_RESPONSE", "default");

    let env = TestEnv::new();
    let result = plugin_coding_pack::executor::execute_workflow_in(
        "test-pr-extraction",
        "add login",
        env.base_dir(),
    )
    .expect("workflow should succeed");

    std::env::remove_var("MOCK_CLAUDE_RESPONSE");

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_step_status(&result, "generate_pr_body", "success");
    assert_step_status(&result, "use_pr_fields", "success");
}
