use pulse_plugin_sdk::error::WitPluginError;
use pulse_plugin_sdk::types::injection::InjectionQuery;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

const DEFAULT_TIMEOUT_SECS: u64 = 300;

// ── Workflow YAML deserialization ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowDef {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub version: serde_yaml::Value,
    #[serde(default)]
    pub requires: Option<Vec<RequiresDef>>,
    pub steps: Vec<StepDef>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequiresDef {
    pub plugin: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StepDef {
    pub id: String,
    #[serde(rename = "type")]
    pub step_type: String,
    #[serde(default)]
    pub executor: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub config: Option<StepConfigDef>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StepConfigDef {
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub user_prompt_template: Option<String>,
    #[serde(default)]
    pub model_tier: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub context_from: Option<Vec<String>>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub retry: Option<RetryConfig>,
    /// Working directory for this step. Supports template variables (e.g. {{working_dir}}).
    /// If not set, falls back to the {{working_dir}} template variable if available.
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// Configuration for iterative fix loops.
/// When a step has a retry config, the executor will re-run it
/// (with test failure context) when the `on_failure_of` step fails.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct RetryConfig {
    pub max_attempts: u32,
    pub on_failure_of: String,
}

// ── Step execution results ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct StepOutput {
    step_id: String,
    status: StepStatus,
    content: Option<String>,
    execution_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum StepStatus {
    Success,
    Failed,
    Skipped,
    Timeout,
}

// ── Public entry point ─────────────────────────────────────────────────────

/// Execute a workflow by ID using default workspace config (current directory).
pub fn execute_workflow(
    workflow_id: &str,
    user_input: &str,
) -> Result<serde_json::Value, WitPluginError> {
    execute_workflow_in(workflow_id, user_input, Path::new("."))
}

/// Execute a workflow with a WorkspaceConfig providing resolved paths and defaults.
pub fn execute_workflow_with_config(
    workflow_id: &str,
    user_input: &str,
    config: &crate::workspace::WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let start = Instant::now();
    let plugins_dir = &config.plugins_dir;
    let workflows_dir = &config.workflows_dir;

    // 1. Load and parse workflow YAML
    let workflow_path = workflows_dir.join(format!("{}.yaml", workflow_id));
    let workflow = load_workflow(&workflow_path)?;

    // 2. Check required plugins
    check_required_plugins(&workflow, plugins_dir)?;

    // 3. Topological sort
    let execution_order = topological_sort(&workflow.steps)?;

    // 4. Execute steps in DAG order
    let mut outputs: HashMap<String, StepOutput> = HashMap::new();
    let mut template_vars: HashMap<String, String> = HashMap::new();
    template_vars.insert("input".to_string(), user_input.to_string());

    // Seed workflow_id and workspace defaults into template vars
    template_vars.insert("workflow_id".to_string(), workflow_id.to_string());
    if let Some(model) = &config.defaults.default_model {
        template_vars.insert("default_model".to_string(), model.clone());
    }
    if let Some(budget) = config.defaults.max_budget_usd {
        template_vars.insert("max_budget_usd".to_string(), budget.to_string());
    }

    execute_workflow_steps(
        workflow_id,
        &workflow,
        &execution_order,
        &mut outputs,
        &mut template_vars,
        plugins_dir,
        start,
        config.use_injection_pipeline,
    )
}

/// Execute a workflow with a configurable base directory.
/// All relative paths (config/plugins, config/workflows) are resolved from `base_dir`.
pub fn execute_workflow_in(
    workflow_id: &str,
    user_input: &str,
    base_dir: &Path,
) -> Result<serde_json::Value, WitPluginError> {
    let config = crate::workspace::WorkspaceConfig::from_base_dir(base_dir);
    execute_workflow_with_config(workflow_id, user_input, &config)
}

/// Internal: run the workflow step loop given pre-resolved state.
#[allow(clippy::too_many_arguments)]
fn execute_workflow_steps(
    workflow_id: &str,
    workflow: &WorkflowDef,
    execution_order: &[String],
    outputs: &mut HashMap<String, StepOutput>,
    template_vars: &mut HashMap<String, String>,
    plugins_dir: &Path,
    start: Instant,
    use_injection_pipeline: bool,
) -> Result<serde_json::Value, WitPluginError> {
    let mut failed_required = false;

    // Build retry index: map from on_failure_of step -> (retryable step id, RetryConfig)
    let retry_index: HashMap<String, (String, RetryConfig)> = workflow
        .steps
        .iter()
        .filter_map(|s| {
            s.config
                .as_ref()
                .and_then(|c| c.retry.as_ref())
                .map(|r| (r.on_failure_of.clone(), (s.id.clone(), r.clone())))
        })
        .collect();

    // Track retry state: step_id -> current attempt number
    let mut retry_attempts: HashMap<String, u32> = HashMap::new();
    // Track retry history for metadata: step_id -> vec of attempt summaries
    let mut retry_history: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

    let mut idx = 0;
    while idx < execution_order.len() {
        let step_id = &execution_order[idx];
        let step = workflow.steps.iter().find(|s| s.id == *step_id).unwrap();

        // Skip remaining steps after a required step failure
        if failed_required {
            outputs.insert(
                step.id.clone(),
                StepOutput {
                    step_id: step.id.clone(),
                    status: StepStatus::Skipped,
                    content: None,
                    execution_time_ms: 0,
                    error: Some("skipped: prior required step failed".to_string()),
                },
            );
            idx += 1;
            continue;
        }

        // Check if all dependencies are satisfied
        if !dependencies_satisfied(step, outputs, &workflow.steps) {
            let output = StepOutput {
                step_id: step.id.clone(),
                status: StepStatus::Skipped,
                content: None,
                execution_time_ms: 0,
                error: Some("dependency not satisfied".to_string()),
            };
            outputs.insert(step.id.clone(), output);
            if !step.optional {
                failed_required = true;
            }
            idx += 1;
            continue;
        }

        eprintln!(
            "[workflow] step {}/{}: {} ({})",
            outputs.len() + 1,
            workflow.steps.len(),
            step.id,
            step.step_type
        );

        let result = execute_step(
            step,
            outputs,
            template_vars,
            plugins_dir,
            use_injection_pipeline,
        );

        match result {
            Ok((mut output, session_id)) => {
                // Extract branch_name from agent output if present
                if let Some(content) = &output.content {
                    if let Some(branch) = extract_branch_name(content) {
                        template_vars.insert("branch_name".to_string(), branch);
                    }
                }

                // Extract PR title/body from PR-generating agent steps
                if output.status == StepStatus::Success && is_pr_generation_step(step) {
                    if let Some(content) = &output.content {
                        let (title, body) = extract_pr_fields(content);
                        if !title.is_empty() {
                            eprintln!("[workflow]   pr: extracted title ({} chars)", title.len());
                            template_vars.insert("pr_title".to_string(), title);
                            template_vars.insert("pr_body".to_string(), body);
                        }
                    }
                }

                // Extract worktree path from git worktree add commands
                if output.status == StepStatus::Success {
                    if let Some(config) = &step.config {
                        if let Some(cmd) = &config.command {
                            if let Some(wt_path) = extract_worktree_path(cmd, template_vars) {
                                eprintln!("[workflow]   worktree: {}", wt_path);
                                template_vars.insert("working_dir".to_string(), wt_path);
                            }
                        }
                    }
                }

                // Forward session_id for continuity between LLM calls
                if let Some(sid) = session_id {
                    template_vars.insert("session_id".to_string(), sid);
                }

                // Quality gate: check review/QA step verdicts
                if output.status == StepStatus::Success && is_review_step(step) {
                    if let Some(content) = &output.content {
                        if let Some(verdict) = extract_verdict(content) {
                            if verdict != "approve" {
                                eprintln!(
                                    "[workflow]   gate: verdict '{}' — blocking downstream",
                                    verdict
                                );
                                output.status = StepStatus::Failed;
                                output.error =
                                    Some(format!("review verdict: {} (commit blocked)", verdict));
                            }
                        }
                    }
                }

                // Retry loop: if this step failed and it's the on_failure_of target for a retryable step
                if output.status == StepStatus::Failed {
                    if let Some((retryable_step_id, retry_cfg)) = retry_index.get(&step.id) {
                        let attempt = retry_attempts.entry(retryable_step_id.clone()).or_insert(1);
                        if *attempt < retry_cfg.max_attempts {
                            *attempt += 1;
                            let failure_context = output.error.as_deref().unwrap_or("");
                            let test_output = output.content.as_deref().unwrap_or("");

                            // Record attempt in retry history
                            retry_history
                                .entry(retryable_step_id.clone())
                                .or_default()
                                .push(serde_json::json!({
                                    "attempt": *attempt - 1,
                                    "status": "failed",
                                    "failure_summary": &failure_context[..failure_context.len().min(500)],
                                }));

                            let retry_context = build_retry_context(
                                *attempt,
                                failure_context,
                                test_output,
                                retry_history.get(retryable_step_id),
                            );
                            template_vars.insert("retry_context".to_string(), retry_context);

                            eprintln!(
                                "[workflow]   retry: re-running '{}' (attempt {}/{})",
                                retryable_step_id, *attempt, retry_cfg.max_attempts
                            );

                            // Find the retryable step's position in execution_order and restart from there
                            if let Some(retry_idx) = execution_order
                                .iter()
                                .position(|id| id == retryable_step_id)
                            {
                                // Clear outputs for the retryable step and the failed test step
                                outputs.remove(retryable_step_id);
                                outputs.remove(&step.id);
                                failed_required = false;
                                idx = retry_idx;
                                continue;
                            }
                        } else {
                            // Retry limit reached
                            eprintln!(
                                "[workflow]   retry: limit reached for '{}' ({} attempts)",
                                retryable_step_id, retry_cfg.max_attempts
                            );
                            output.error = Some(format!(
                                "retry_limit_reached: {} attempts exhausted. Last error: {}",
                                retry_cfg.max_attempts,
                                output.error.as_deref().unwrap_or("unknown")
                            ));
                        }
                    }
                }

                if output.status != StepStatus::Success && !step.optional {
                    failed_required = true;
                }

                eprintln!(
                    "[workflow]   → {:?} ({}ms)",
                    output.status, output.execution_time_ms
                );

                // Attach retry metadata if applicable
                let step_output = if let Some(history) = retry_history.get(&step.id) {
                    let mut enriched = output.clone();
                    if let Some(content) = &enriched.content {
                        let attempts = retry_attempts.get(&step.id).copied().unwrap_or(1);
                        enriched.content = Some(format!(
                            "{}\n\n<!-- retry_metadata: {{\"attempts\": {}, \"history\": {}}} -->",
                            content,
                            attempts,
                            serde_json::to_string(history).unwrap_or_default()
                        ));
                    }
                    enriched
                } else {
                    output
                };
                outputs.insert(step.id.clone(), step_output);
            }
            Err(e) => {
                eprintln!("[workflow]   → error: {}", e.message);
                let output = StepOutput {
                    step_id: step.id.clone(),
                    status: StepStatus::Failed,
                    content: None,
                    execution_time_ms: 0,
                    error: Some(e.message.clone()),
                };
                if !step.optional {
                    failed_required = true;
                }
                outputs.insert(step.id.clone(), output);
            }
        }

        idx += 1;
    }

    // 5. Build result
    let total_time = start.elapsed().as_millis() as u64;
    let steps_completed = outputs
        .values()
        .filter(|o| o.status == StepStatus::Success)
        .count();

    let overall_status = if failed_required {
        "failed"
    } else if outputs
        .values()
        .any(|o| o.status == StepStatus::Skipped || o.status == StepStatus::Timeout)
    {
        "partial"
    } else {
        "completed"
    };

    let step_results: Vec<serde_json::Value> = execution_order
        .iter()
        .filter_map(|id| outputs.get(id))
        .map(|o| {
            let mut v = serde_json::json!({
                "step_id": o.step_id,
                "status": o.status,
                "execution_time_ms": o.execution_time_ms,
            });
            if let Some(err) = &o.error {
                v["error"] = serde_json::json!(err);
            }
            if let Some(content) = &o.content {
                let preview: String = content.chars().take(200).collect();
                v["content_preview"] = serde_json::json!(preview);
            }
            v
        })
        .collect();

    Ok(serde_json::json!({
        "workflow_id": workflow_id,
        "status": overall_status,
        "steps_completed": steps_completed,
        "steps_total": workflow.steps.len(),
        "steps": step_results,
        "total_execution_time_ms": total_time,
    }))
}

// ── Workflow loading ───────────────────────────────────────────────────────

fn load_workflow(path: &Path) -> Result<WorkflowDef, WitPluginError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        WitPluginError::not_found(format!("workflow not found: {}: {}", path.display(), e))
    })?;

    serde_yaml::from_str(&content)
        .map_err(|e| WitPluginError::invalid_input(format!("invalid workflow YAML: {}", e)))
}

// ── Plugin availability ────────────────────────────────────────────────────

fn check_required_plugins(
    workflow: &WorkflowDef,
    plugins_dir: &Path,
) -> Result<(), WitPluginError> {
    if let Some(requires) = &workflow.requires {
        for req in requires {
            let plugin_path = plugins_dir.join(&req.plugin);
            if !plugin_path.exists() && !req.optional {
                return Err(WitPluginError::not_found(format!(
                    "required plugin '{}' not found at {}",
                    req.plugin,
                    plugin_path.display()
                )));
            }
        }
    }
    Ok(())
}

// ── Topological sort (Kahn's algorithm) ────────────────────────────────────

/// Returns a flat ordering for sequential execution.
fn topological_sort(steps: &[StepDef]) -> Result<Vec<String>, WitPluginError> {
    let levels = topological_sort_levels(steps)?;
    Ok(levels.into_iter().flatten().collect())
}

/// Returns steps grouped by dependency level for parallel execution.
/// Each inner Vec contains steps that can run concurrently.
fn topological_sort_levels(steps: &[StepDef]) -> Result<Vec<Vec<String>>, WitPluginError> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in steps {
        in_degree.entry(step.id.as_str()).or_insert(0);
        adj.entry(step.id.as_str()).or_default();
        for dep in &step.depends_on {
            adj.entry(dep.as_str()).or_default().push(step.id.as_str());
            *in_degree.entry(step.id.as_str()).or_insert(0) += 1;
        }
    }

    let mut levels: Vec<Vec<String>> = Vec::new();
    let mut total_processed = 0;

    // Seed with zero-degree nodes
    let mut current_level: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();
    current_level.sort(); // deterministic ordering

    while !current_level.is_empty() {
        let level: Vec<String> = current_level.iter().map(|s| s.to_string()).collect();
        total_processed += level.len();

        // Find next level: reduce in-degree for neighbors
        let mut next_level: Vec<&str> = Vec::new();
        for &node in &current_level {
            if let Some(neighbors) = adj.get(node) {
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            next_level.push(neighbor);
                        }
                    }
                }
            }
        }
        next_level.sort();

        levels.push(level);
        current_level = next_level;
    }

    if total_processed != steps.len() {
        return Err(WitPluginError::invalid_input(
            "workflow DAG contains a cycle",
        ));
    }

    Ok(levels)
}

// ── Dependency checking ────────────────────────────────────────────────────

fn dependencies_satisfied(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    all_steps: &[StepDef],
) -> bool {
    for dep_id in &step.depends_on {
        match outputs.get(dep_id.as_str()) {
            None => return false,
            Some(output) if output.status == StepStatus::Success => continue,
            Some(_) => {
                // Dep failed/skipped/timed out — only allow if the dep is optional
                let dep_optional = all_steps
                    .iter()
                    .find(|s| s.id == *dep_id)
                    .is_some_and(|s| s.optional);
                if !dep_optional {
                    return false;
                }
            }
        }
    }
    true
}

// ── Template variable substitution ─────────────────────────────────────────

fn substitute_templates(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

// ── Verdict extraction (quality gate) ──────────────────────────────────────

/// Extract a review verdict from agent output.
/// Looks for `verdict: approve|request-changes|reject`.
/// Returns None if no verdict line found (treated as approve for backwards compat).
fn extract_verdict(content: &str) -> Option<String> {
    for line in content.lines().rev() {
        let trimmed = line.trim().to_lowercase();
        if let Some(rest) = trimmed.strip_prefix("verdict:") {
            let verdict = rest.trim().trim_matches('"').trim_matches('\'');
            match verdict {
                "approve" | "request-changes" | "reject" => {
                    return Some(verdict.to_string());
                }
                _ => continue,
            }
        }
    }
    None
}

/// Check if a step is a review/QA step based on its system_prompt keywords.
fn is_review_step(step: &StepDef) -> bool {
    if step.step_type != "agent" {
        return false;
    }
    let prompt = step
        .config
        .as_ref()
        .and_then(|c| c.system_prompt.as_deref())
        .unwrap_or("");
    let lower = prompt.to_lowercase();
    lower.contains("review") || lower.contains("qa")
}

/// Check if a step is a PR-generating agent step (system_prompt mentions "pull request" or "PR").
fn is_pr_generation_step(step: &StepDef) -> bool {
    if step.step_type != "agent" {
        return false;
    }
    let prompt = step
        .config
        .as_ref()
        .and_then(|c| c.system_prompt.as_deref())
        .unwrap_or("");
    let lower = prompt.to_lowercase();
    lower.contains("pull request") || lower.contains("pr description") || lower.contains("pr body")
}

/// Extract worktree path from a git worktree add command.
/// Returns the resolved path if the command contains "worktree" and "add".
fn extract_worktree_path(
    command: &[String],
    template_vars: &HashMap<String, String>,
) -> Option<String> {
    let joined = command.join(" ").to_lowercase();
    if !joined.contains("worktree") || !joined.contains("add") {
        return None;
    }
    // The worktree path is the last positional argument (not a flag)
    // Resolve template variables first
    let resolved: Vec<String> = command
        .iter()
        .map(|part| substitute_templates(part, template_vars))
        .collect();
    // Find last arg that doesn't start with '-' and isn't "git"/"worktree"/"add"
    resolved
        .iter()
        .rev()
        .find(|arg| !arg.starts_with('-') && !["git", "worktree", "add"].contains(&arg.as_str()))
        .cloned()
}

// ── PR fields extraction (for auto-generated PRs) ─────────────────────────

/// Extract PR title (first line, max 72 chars) and body (rest) from agent output.
/// Used by workflow templates when generating PR content from agent output.
pub fn extract_pr_fields(content: &str) -> (String, String) {
    let parts: Vec<&str> = content.splitn(2, "\n\n").collect();
    let title = parts[0].trim();
    let title = if title.len() > 72 {
        format!("{}...", &title[..69])
    } else {
        title.to_string()
    };
    let body = if parts.len() > 1 {
        parts[1].trim().to_string()
    } else {
        String::new()
    };
    (title, body)
}

// ── Branch name extraction ─────────────────────────────────────────────────

fn extract_branch_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("branch_name:") {
            let name = rest.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

// ── Retry context builder (progressive enrichment) ─────────────────────────

/// Build enriched context for retry attempts, including prior failure details.
fn build_retry_context(
    attempt: u32,
    failure_error: &str,
    test_output: &str,
    history: Option<&Vec<serde_json::Value>>,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!("## Retry Attempt {} — Fix Required", attempt));

    if !failure_error.is_empty() {
        parts.push(format!(
            "### Failure Details\n{}",
            &failure_error[..failure_error.len().min(2000)]
        ));
    }

    if !test_output.is_empty() {
        parts.push(format!(
            "### Test Output\n```\n{}\n```",
            &test_output[..test_output.len().min(2000)]
        ));
    }

    if attempt == 2 {
        parts.push(
            "### Instructions\nYour previous implementation failed the tests above. \
             Fix the issues while preserving working functionality."
                .to_string(),
        );
    } else if attempt >= 3 {
        if let Some(hist) = history {
            let recurring: Vec<String> = hist
                .iter()
                .filter_map(|h| {
                    h.get("failure_summary")
                        .and_then(|s| s.as_str())
                        .map(String::from)
                })
                .collect();
            if !recurring.is_empty() {
                parts.push(format!(
                    "### Failure Pattern Analysis\nRecurring failures across {} attempts:\n{}",
                    hist.len(),
                    recurring
                        .iter()
                        .enumerate()
                        .map(|(i, f)| format!("  Attempt {}: {}", i + 1, f))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }
        }
        parts.push(
            "### Instructions\nMultiple attempts have failed. Carefully analyze all prior failures \
             and take a different approach if the same issues recur."
                .to_string(),
        );
    }

    parts.join("\n\n")
}

// ── Context assembly from prior steps ──────────────────────────────────────

fn assemble_context(step: &StepDef, outputs: &HashMap<String, StepOutput>) -> String {
    let context_ids = match step.config.as_ref().and_then(|c| c.context_from.as_ref()) {
        Some(ids) => ids,
        None => return String::new(),
    };

    let mut parts = Vec::new();
    for ctx_id in context_ids {
        if let Some(output) = outputs.get(ctx_id.as_str()) {
            if let Some(content) = &output.content {
                parts.push(format!(
                    "--- Output from step '{}' ---\n{}",
                    ctx_id, content
                ));
            }
        }
    }
    parts.join("\n\n")
}

// ── Step execution dispatch ────────────────────────────────────────────────

/// Returns (StepOutput, Option<session_id>)
fn execute_step(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    template_vars: &HashMap<String, String>,
    plugins_dir: &Path,
    use_injection_pipeline: bool,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let timeout_secs = step
        .config
        .as_ref()
        .and_then(|c| c.timeout_seconds)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    match step.step_type.as_str() {
        "agent" => execute_agent_step(
            step,
            outputs,
            template_vars,
            timeout_secs,
            plugins_dir,
            use_injection_pipeline,
        ),
        "function" => {
            execute_function_step(step, template_vars, timeout_secs, plugins_dir).map(|o| (o, None))
        }
        other => Err(WitPluginError::invalid_input(format!(
            "step '{}': unknown step type '{}'",
            step.id, other
        ))),
    }
}

// ── Agent step execution ────────────────────────────────────────────────────
//
// Two execution patterns:
//   1. executor: bmad-method  → two-stage: fetch persona, then call provider-claude-code
//   2. executor: provider-claude-code → direct: call with workflow system_prompt
//
// Session continuity: provider-claude-code returns session_id which is forwarded
// to subsequent calls via template_vars["session_id"].

#[allow(clippy::too_many_arguments)]
fn execute_agent_step(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    template_vars: &HashMap<String, String>,
    timeout_secs: u64,
    plugins_dir: &Path,
    use_injection_pipeline: bool,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let start = Instant::now();

    let executor = step.executor.as_deref().ok_or_else(|| {
        WitPluginError::invalid_input(format!(
            "step '{}': agent step requires 'executor'",
            step.id
        ))
    })?;

    let config = step.config.as_ref().ok_or_else(|| {
        WitPluginError::invalid_input(format!("step '{}': agent step requires 'config'", step.id))
    })?;

    // Build the prompt with context injection
    let context = assemble_context(step, outputs);
    let user_prompt = config
        .user_prompt_template
        .as_deref()
        .map(|t| substitute_templates(t, template_vars))
        .unwrap_or_else(|| template_vars.get("input").cloned().unwrap_or_default());

    let full_prompt = if context.is_empty() {
        user_prompt
    } else {
        format!(
            "{}\n\n## Context from prior steps:\n{}",
            user_prompt, context
        )
    };

    if executor == "bmad-method" {
        execute_bmad_agent_step(
            step,
            &full_prompt,
            config,
            template_vars,
            timeout_secs,
            start,
            plugins_dir,
            use_injection_pipeline,
        )
    } else {
        execute_direct_agent_step(
            step,
            executor,
            &full_prompt,
            config,
            template_vars,
            timeout_secs,
            start,
            plugins_dir,
        )
    }
}

/// Two-stage execution for bmad-method agent steps:
/// 1. Call bmad-method to get agent persona (system prompt + config)
/// 2. Call provider-claude-code with that persona to do actual LLM reasoning
///
/// When `use_injection_pipeline` is true, Stage 1 is skipped and the injection
/// pipeline (BmadAgentInjector) handles system prompt composition.
#[allow(clippy::too_many_arguments)]
fn execute_bmad_agent_step(
    step: &StepDef,
    prompt: &str,
    config: &StepConfigDef,
    template_vars: &HashMap<String, String>,
    timeout_secs: u64,
    start: Instant,
    plugins_dir: &Path,
    use_injection_pipeline: bool,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let bmad_binary = plugins_dir.join("bmad-method");
    if !bmad_binary.exists() {
        return Err(WitPluginError::not_found(format!(
            "step '{}': bmad-method not found at {}",
            step.id,
            bmad_binary.display()
        )));
    }

    let claude_binary = plugins_dir.join("provider-claude-code");
    if !claude_binary.exists() {
        return Err(WitPluginError::not_found(format!(
            "step '{}': provider-claude-code not found (required for bmad agent execution)",
            step.id
        )));
    }

    let agent_name = extract_agent_name(config);

    let injection_query = {
        let mut query = InjectionQuery::new();
        if agent_name.starts_with("bmad/") {
            query.agent_name = Some(agent_name.clone());
        }
        query.workflow_name = template_vars.get("workflow_id").cloned();
        query.step_type = Some("agent".to_string());
        query
    };

    let (merged_system_prompt, model_tier) = if use_injection_pipeline {
        // New path: injection pipeline handles system prompt composition
        eprintln!("[workflow]   injection pipeline active — skipping persona fetch");
        let model_tier = config
            .model_tier
            .as_deref()
            .unwrap_or("balanced")
            .to_string();
        (None, model_tier)
    } else {
        // Legacy two-stage path — retained for backward compatibility when use_injection_pipeline is false
        eprintln!("[workflow]   stage 1: fetching persona '{}'", agent_name);

        let persona_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "step-executor.execute",
            "params": {
                "task": {
                    "task_id": format!("wf-{}-persona", step.id),
                    "description": "fetch agent persona",
                    "input": {
                        "agent": agent_name,
                        "prompt": prompt,
                    },
                },
                "config": {
                    "step_id": format!("{}-persona", step.id),
                    "step_type": "agent",
                }
            }
        });

        let persona_response = spawn_plugin_rpc(&bmad_binary, &persona_request, 30)?;
        let persona_content = extract_rpc_content(&persona_response)?;

        // Parse the persona JSON to extract the agent's system_prompt
        let persona: serde_json::Value = serde_json::from_str(&persona_content).map_err(|e| {
            WitPluginError::internal(format!(
                "step '{}': failed to parse persona JSON: {}",
                step.id, e
            ))
        })?;

        let agent_system_prompt = persona
            .get("system_prompt")
            .and_then(|s| s.as_str())
            .unwrap_or("");

        // Merge: workflow system_prompt provides task-specific instructions,
        // persona system_prompt provides the agent's character/role
        let merged = if let Some(workflow_sp) = &config.system_prompt {
            format!(
                "{}\n\n## Task-Specific Instructions\n{}",
                agent_system_prompt, workflow_sp
            )
        } else {
            agent_system_prompt.to_string()
        };

        // Extract suggested config from persona
        let suggested_model = persona
            .get("suggested_config")
            .and_then(|c| c.get("model_tier"))
            .and_then(|m| m.as_str());

        let model_tier = config
            .model_tier
            .as_deref()
            .or(suggested_model)
            .unwrap_or("balanced")
            .to_string();

        (Some(merged), model_tier)
    };

    // Stage 2: Call provider-claude-code with the persona
    eprintln!(
        "[workflow]   stage 2: executing via provider-claude-code (model: {})",
        model_tier
    );

    let mut parameters = serde_json::Map::new();
    if let Some(ref sp) = merged_system_prompt {
        parameters.insert("system_prompt".to_string(), serde_json::json!(sp));
    }
    parameters.insert("model_tier".to_string(), serde_json::json!(model_tier));
    if let Some(tokens) = config.max_tokens {
        parameters.insert("max_tokens".to_string(), serde_json::json!(tokens));
    }
    // Forward session_id for continuity
    if let Some(session_id) = template_vars.get("session_id") {
        parameters.insert("session_id".to_string(), serde_json::json!(session_id));
    }
    // Forward working_dir so Claude Code runs in the right directory
    let work_dir = config
        .working_dir
        .as_deref()
        .map(|wd| substitute_templates(wd, template_vars))
        .or_else(|| template_vars.get("working_dir").cloned());
    if let Some(ref wd) = work_dir {
        parameters.insert("working_dir".to_string(), serde_json::json!(wd));
    }
    // Forward injection query so the engine can invoke the injection pipeline
    if use_injection_pipeline {
        if let Ok(iq_val) = serde_json::to_value(&injection_query) {
            parameters.insert("injection_query".to_string(), iq_val);
        }
    }

    let claude_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "step-executor.execute",
        "params": {
            "task": {
                "task_id": format!("wf-{}", step.id),
                "description": prompt,
                "input": {
                    "prompt": prompt,
                },
            },
            "config": {
                "step_id": step.id,
                "step_type": "agent",
                "timeout_secs": config.timeout_seconds.unwrap_or(timeout_secs),
                "parameters": parameters,
            }
        }
    });

    let response = spawn_plugin_rpc(&claude_binary, &claude_request, timeout_secs)?;
    let elapsed = start.elapsed().as_millis() as u64;

    parse_claude_response(&step.id, response, elapsed)
}

/// Direct execution for provider-claude-code (or other non-bmad executors)
#[allow(clippy::too_many_arguments)]
fn execute_direct_agent_step(
    step: &StepDef,
    executor: &str,
    prompt: &str,
    config: &StepConfigDef,
    template_vars: &HashMap<String, String>,
    timeout_secs: u64,
    start: Instant,
    plugins_dir: &Path,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let plugin_binary = plugins_dir.join(executor);
    if !plugin_binary.exists() {
        return Err(WitPluginError::not_found(format!(
            "step '{}': executor '{}' not found at {}",
            step.id,
            executor,
            plugin_binary.display()
        )));
    }

    let injection_query = {
        let mut query = InjectionQuery::new();
        // No BMAD agent name for direct execution
        query.workflow_name = template_vars.get("workflow_id").cloned();
        query.step_type = Some("agent".to_string());
        query
    };

    let mut parameters = serde_json::Map::new();
    if let Some(sp) = &config.system_prompt {
        parameters.insert("system_prompt".to_string(), serde_json::json!(sp));
    }
    if let Some(tier) = &config.model_tier {
        parameters.insert("model_tier".to_string(), serde_json::json!(tier));
    }
    if let Some(tokens) = config.max_tokens {
        parameters.insert("max_tokens".to_string(), serde_json::json!(tokens));
    }
    if let Some(session_id) = template_vars.get("session_id") {
        parameters.insert("session_id".to_string(), serde_json::json!(session_id));
    }
    // Forward working_dir
    let work_dir = config
        .working_dir
        .as_deref()
        .map(|wd| substitute_templates(wd, template_vars))
        .or_else(|| template_vars.get("working_dir").cloned());
    if let Some(ref wd) = work_dir {
        parameters.insert("working_dir".to_string(), serde_json::json!(wd));
    }
    // Forward injection query for the engine's injection pipeline
    if let Ok(iq_val) = serde_json::to_value(&injection_query) {
        parameters.insert("injection_query".to_string(), iq_val);
    }

    let rpc_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "step-executor.execute",
        "params": {
            "task": {
                "task_id": format!("wf-{}", step.id),
                "description": prompt,
                "input": {
                    "prompt": prompt,
                },
            },
            "config": {
                "step_id": step.id,
                "step_type": step.step_type,
                "timeout_secs": config.timeout_seconds.unwrap_or(timeout_secs),
                "parameters": parameters,
            }
        }
    });

    let response = spawn_plugin_rpc(&plugin_binary, &rpc_request, timeout_secs)?;
    let elapsed = start.elapsed().as_millis() as u64;

    parse_claude_response(&step.id, response, elapsed)
}

/// Extract agent name (e.g. "bmad/architect") from workflow step config
fn extract_agent_name(config: &StepConfigDef) -> String {
    config
        .system_prompt
        .as_deref()
        .and_then(|sp| {
            sp.split_whitespace()
                .find(|w| w.starts_with("bmad/"))
                .map(|w| {
                    w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '-')
                        .to_string()
                })
        })
        .unwrap_or_else(|| "bmad/architect".to_string())
}

/// Extract the content string from a JSON-RPC response
fn extract_rpc_content(response: &serde_json::Value) -> Result<String, WitPluginError> {
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(WitPluginError::internal(msg.to_string()));
    }

    response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| WitPluginError::internal("no content in plugin response".to_string()))
}

/// Parse provider-claude-code response, extracting the actual result text
/// and session_id from the nested JSON content.
/// Returns (StepOutput, Option<session_id>).
fn parse_claude_response(
    step_id: &str,
    response: serde_json::Value,
    elapsed_ms: u64,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    // Check for JSON-RPC error
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Ok((
            StepOutput {
                step_id: step_id.to_string(),
                status: StepStatus::Failed,
                content: None,
                execution_time_ms: elapsed_ms,
                error: Some(msg.to_string()),
            },
            None,
        ));
    }

    let result = response
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let status_str = result
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    if status_str != "success" {
        return Ok((
            StepOutput {
                step_id: step_id.to_string(),
                status: StepStatus::Failed,
                content: None,
                execution_time_ms: elapsed_ms,
                error: Some(format!("step returned status: {}", status_str)),
            },
            None,
        ));
    }

    // provider-claude-code returns content as a JSON string:
    // {"result": "actual text", "session_id": "...", "total_cost_usd": 0.01}
    let content_str = result.get("content").and_then(|c| c.as_str()).unwrap_or("");

    // Try to parse the inner JSON to extract actual result text and session_id
    let (actual_content, session_id) =
        if let Ok(inner) = serde_json::from_str::<serde_json::Value>(content_str) {
            let text = inner
                .get("result")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| content_str.to_string());
            let sid = inner
                .get("session_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            (text, sid)
        } else {
            (content_str.to_string(), None)
        };

    Ok((
        StepOutput {
            step_id: step_id.to_string(),
            status: StepStatus::Success,
            content: if actual_content.is_empty() {
                None
            } else {
                Some(actual_content)
            },
            execution_time_ms: elapsed_ms,
            error: None,
        },
        session_id,
    ))
}

// ── Function step execution (direct command) ───────────────────────────────

fn execute_function_step(
    step: &StepDef,
    template_vars: &HashMap<String, String>,
    timeout_secs: u64,
    plugins_dir: &Path,
) -> Result<StepOutput, WitPluginError> {
    let start = Instant::now();

    let config = step.config.as_ref().ok_or_else(|| {
        WitPluginError::invalid_input(format!(
            "step '{}': function step requires 'config'",
            step.id
        ))
    })?;

    let command = config.command.as_ref().ok_or_else(|| {
        WitPluginError::invalid_input(format!(
            "step '{}': function step requires 'command'",
            step.id
        ))
    })?;

    if command.is_empty() {
        return Err(WitPluginError::invalid_input(format!(
            "step '{}': command array is empty",
            step.id
        )));
    }

    // Substitute template variables in all command parts
    let resolved_cmd: Vec<String> = command
        .iter()
        .map(|part| substitute_templates(part, template_vars))
        .collect();

    // Resolve program path: check config/plugins/ for command[0], then use PATH.
    // NOTE: The `executor` field on function steps is for dependency validation only
    // (checked by check_required_plugins). The actual program comes from command[0].
    let program = {
        let plugin_path = plugins_dir.join(&resolved_cmd[0]);
        if plugin_path.exists() {
            plugin_path.to_string_lossy().to_string()
        } else {
            resolved_cmd[0].clone()
        }
    };

    // Resolve working directory: step config > template var > inherit
    let work_dir = config
        .working_dir
        .as_deref()
        .map(|wd| substitute_templates(wd, template_vars))
        .or_else(|| template_vars.get("working_dir").cloned());

    let mut cmd = Command::new(&program);
    cmd.args(&resolved_cmd[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(ref wd) = work_dir {
        let wd_path = Path::new(wd);
        if wd_path.exists() {
            cmd.current_dir(wd_path);
            eprintln!("[workflow]   cwd: {}", wd);
        }
    }

    let child = cmd.spawn().map_err(|e| {
        WitPluginError::internal(format!(
            "step '{}': failed to spawn '{}': {}",
            step.id, program, e
        ))
    })?;

    // Timeout watchdog
    let child_id = child.id();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_clone = Arc::clone(&timed_out);
    let (tx, rx) = std::sync::mpsc::channel();

    let watchdog = std::thread::spawn(move || {
        if rx.recv_timeout(timeout).is_err() {
            timed_out_clone.store(true, Ordering::SeqCst);
            let _ = Command::new("kill")
                .args(["-9", &child_id.to_string()])
                .status();
        }
    });

    let output = child.wait_with_output().map_err(|e| {
        WitPluginError::internal(format!(
            "step '{}': failed to wait for process: {}",
            step.id, e
        ))
    })?;

    let _ = tx.send(());
    let _ = watchdog.join();

    let elapsed = start.elapsed().as_millis() as u64;

    if timed_out.load(Ordering::SeqCst) {
        return Ok(StepOutput {
            step_id: step.id.clone(),
            status: StepStatus::Timeout,
            content: None,
            execution_time_ms: elapsed,
            error: Some(format!("timed out after {}s", timeout_secs)),
        });
    }

    let stdout_content = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_content = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(StepOutput {
            step_id: step.id.clone(),
            status: StepStatus::Success,
            content: if stdout_content.is_empty() {
                None
            } else {
                Some(stdout_content)
            },
            execution_time_ms: elapsed,
            error: None,
        })
    } else {
        let exit_code = output.status.code().unwrap_or(-1);
        Ok(StepOutput {
            step_id: step.id.clone(),
            status: StepStatus::Failed,
            content: if stdout_content.is_empty() {
                None
            } else {
                Some(stdout_content)
            },
            execution_time_ms: elapsed,
            error: Some(format!(
                "exit code {}: {}",
                exit_code,
                stderr_content.chars().take(500).collect::<String>()
            )),
        })
    }
}

// ── JSON-RPC communication with plugin child processes ─────────────────────

fn spawn_plugin_rpc(
    binary_path: &Path,
    request: &serde_json::Value,
    timeout_secs: u64,
) -> Result<serde_json::Value, WitPluginError> {
    let mut child = Command::new(binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            WitPluginError::internal(format!(
                "failed to spawn plugin {}: {}",
                binary_path.display(),
                e
            ))
        })?;

    // Write JSON-RPC request to stdin then close it
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| WitPluginError::internal("failed to open stdin for plugin"))?;
        serde_json::to_writer(&mut *stdin, request)
            .map_err(|e| WitPluginError::internal(format!("failed to write RPC request: {}", e)))?;
        writeln!(stdin)
            .map_err(|e| WitPluginError::internal(format!("failed to write newline: {}", e)))?;
        stdin
            .flush()
            .map_err(|e| WitPluginError::internal(format!("failed to flush stdin: {}", e)))?;
    }
    drop(child.stdin.take());

    // Timeout watchdog
    let child_id = child.id();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_clone = Arc::clone(&timed_out);
    let (tx, rx) = std::sync::mpsc::channel();

    let watchdog = std::thread::spawn(move || {
        if rx.recv_timeout(timeout).is_err() {
            timed_out_clone.store(true, Ordering::SeqCst);
            let _ = Command::new("kill")
                .args(["-9", &child_id.to_string()])
                .status();
        }
    });

    // Read stdout for JSON-RPC response
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WitPluginError::internal("failed to open stdout for plugin"))?;
    let reader = BufReader::new(stdout);

    let mut response: Option<serde_json::Value> = None;
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Accept messages with "result" or "error" (JSON-RPC responses)
            if (msg.get("result").is_some() || msg.get("error").is_some())
                && msg.get("method").is_none()
            {
                response = Some(msg);
                break;
            }
        }
    }

    let _ = tx.send(());
    let _ = child.wait();
    let _ = watchdog.join();

    if timed_out.load(Ordering::SeqCst) {
        return Err(WitPluginError::internal("plugin execution timed out"));
    }

    response.ok_or_else(|| WitPluginError::internal("plugin returned no JSON-RPC response"))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── substitute_templates ───────────────────────────────────────────

    #[test]
    fn substitute_templates_replaces_input() {
        let mut vars = HashMap::new();
        vars.insert("input".to_string(), "hello world".to_string());
        assert_eq!(
            substitute_templates("do: {{input}}", &vars),
            "do: hello world"
        );
    }

    #[test]
    fn substitute_templates_replaces_multiple_vars() {
        let mut vars = HashMap::new();
        vars.insert("input".to_string(), "task".to_string());
        vars.insert("branch_name".to_string(), "feat-login".to_string());
        let result = substitute_templates(
            "git checkout -b feature/{{branch_name}} for {{input}}",
            &vars,
        );
        assert_eq!(result, "git checkout -b feature/feat-login for task");
    }

    #[test]
    fn substitute_templates_no_match_unchanged() {
        let vars = HashMap::new();
        assert_eq!(substitute_templates("no vars here", &vars), "no vars here");
    }

    #[test]
    fn substitute_templates_unresolved_var_left_as_is() {
        let vars = HashMap::new();
        assert_eq!(substitute_templates("{{unknown}}", &vars), "{{unknown}}");
    }

    // ── extract_branch_name ────────────────────────────────────────────

    #[test]
    fn extract_branch_name_simple() {
        let content = "some output\nbranch_name: add-login\nmore text";
        assert_eq!(extract_branch_name(content), Some("add-login".to_string()));
    }

    #[test]
    fn extract_branch_name_quoted() {
        let content = "branch_name: \"my-feature\"";
        assert_eq!(extract_branch_name(content), Some("my-feature".to_string()));
    }

    #[test]
    fn extract_branch_name_with_spaces() {
        let content = "  branch_name:   feat-xyz  ";
        assert_eq!(extract_branch_name(content), Some("feat-xyz".to_string()));
    }

    #[test]
    fn extract_branch_name_not_found() {
        let content = "no branch info here\njust regular text";
        assert_eq!(extract_branch_name(content), None);
    }

    // ── topological_sort ───────────────────────────────────────────────

    fn make_step(id: &str, deps: &[&str]) -> StepDef {
        StepDef {
            id: id.to_string(),
            step_type: "function".to_string(),
            executor: None,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            optional: false,
            config: None,
        }
    }

    #[test]
    fn topological_sort_linear() {
        let steps = vec![
            make_step("a", &[]),
            make_step("b", &["a"]),
            make_step("c", &["b"]),
        ];
        let order = topological_sort(&steps).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn topological_sort_parallel_then_join() {
        let steps = vec![
            make_step("a", &[]),
            make_step("b", &[]),
            make_step("c", &["a", "b"]),
        ];
        let order = topological_sort(&steps).unwrap();
        // a and b are both roots, sorted alphabetically
        assert_eq!(order[0], "a");
        assert_eq!(order[1], "b");
        assert_eq!(order[2], "c");
    }

    #[test]
    fn topological_sort_diamond() {
        let steps = vec![
            make_step("a", &[]),
            make_step("b", &["a"]),
            make_step("c", &["a"]),
            make_step("d", &["b", "c"]),
        ];
        let order = topological_sort(&steps).unwrap();
        assert_eq!(order[0], "a");
        assert_eq!(order[3], "d");
        // b and c can be in either order but both before d
        assert!(order.contains(&"b".to_string()));
        assert!(order.contains(&"c".to_string()));
    }

    #[test]
    fn topological_sort_cycle_detected() {
        let steps = vec![make_step("a", &["b"]), make_step("b", &["a"])];
        assert!(topological_sort(&steps).is_err());
    }

    // ── topological_sort_levels (parallel execution) ─────────────────

    #[test]
    fn topological_sort_levels_linear() {
        let steps = vec![
            make_step("a", &[]),
            make_step("b", &["a"]),
            make_step("c", &["b"]),
        ];
        let levels = topological_sort_levels(&steps).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["a"]);
        assert_eq!(levels[1], vec!["b"]);
        assert_eq!(levels[2], vec!["c"]);
    }

    #[test]
    fn topological_sort_levels_parallel_roots() {
        let steps = vec![
            make_step("a", &[]),
            make_step("b", &[]),
            make_step("c", &["a", "b"]),
        ];
        let levels = topological_sort_levels(&steps).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0], vec!["a", "b"]); // parallel level
        assert_eq!(levels[1], vec!["c"]);
    }

    #[test]
    fn topological_sort_levels_diamond() {
        let steps = vec![
            make_step("a", &[]),
            make_step("b", &["a"]),
            make_step("c", &["a"]),
            make_step("d", &["b", "c"]),
        ];
        let levels = topological_sort_levels(&steps).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["a"]);
        assert_eq!(levels[1], vec!["b", "c"]); // parallel level
        assert_eq!(levels[2], vec!["d"]);
    }

    #[test]
    fn topological_sort_levels_review_parallel() {
        // Simulates coding-review: two reviews depend on same step
        let steps = vec![
            make_step("implement", &[]),
            make_step("adversarial_review", &["implement"]),
            make_step("edge_case_review", &["implement"]),
            make_step("synthesis", &["adversarial_review", "edge_case_review"]),
        ];
        let levels = topological_sort_levels(&steps).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["implement"]);
        assert_eq!(levels[1], vec!["adversarial_review", "edge_case_review"]);
        assert_eq!(levels[2], vec!["synthesis"]);
    }

    // ── dependencies_satisfied ─────────────────────────────────────────

    #[test]
    fn deps_satisfied_all_success() {
        let steps = vec![make_step("a", &[]), make_step("b", &["a"])];
        let mut outputs = HashMap::new();
        outputs.insert(
            "a".to_string(),
            StepOutput {
                step_id: "a".to_string(),
                status: StepStatus::Success,
                content: None,
                execution_time_ms: 0,
                error: None,
            },
        );
        assert!(dependencies_satisfied(&steps[1], &outputs, &steps));
    }

    #[test]
    fn deps_satisfied_optional_failed_is_ok() {
        let mut steps = vec![make_step("a", &[]), make_step("b", &["a"])];
        steps[0].optional = true;

        let mut outputs = HashMap::new();
        outputs.insert(
            "a".to_string(),
            StepOutput {
                step_id: "a".to_string(),
                status: StepStatus::Failed,
                content: None,
                execution_time_ms: 0,
                error: Some("failed".to_string()),
            },
        );
        assert!(dependencies_satisfied(&steps[1], &outputs, &steps));
    }

    #[test]
    fn deps_satisfied_required_failed_blocks() {
        let steps = vec![make_step("a", &[]), make_step("b", &["a"])];
        let mut outputs = HashMap::new();
        outputs.insert(
            "a".to_string(),
            StepOutput {
                step_id: "a".to_string(),
                status: StepStatus::Failed,
                content: None,
                execution_time_ms: 0,
                error: Some("failed".to_string()),
            },
        );
        assert!(!dependencies_satisfied(&steps[1], &outputs, &steps));
    }

    // ── assemble_context ───────────────────────────────────────────────

    #[test]
    fn assemble_context_empty_when_no_context_from() {
        let step = make_step("a", &[]);
        let outputs = HashMap::new();
        assert_eq!(assemble_context(&step, &outputs), "");
    }

    #[test]
    fn assemble_context_concatenates_outputs() {
        let mut step = make_step("c", &["a", "b"]);
        step.config = Some(StepConfigDef {
            context_from: Some(vec!["a".to_string(), "b".to_string()]),
            system_prompt: None,
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        });

        let mut outputs = HashMap::new();
        outputs.insert(
            "a".to_string(),
            StepOutput {
                step_id: "a".to_string(),
                status: StepStatus::Success,
                content: Some("output A".to_string()),
                execution_time_ms: 0,
                error: None,
            },
        );
        outputs.insert(
            "b".to_string(),
            StepOutput {
                step_id: "b".to_string(),
                status: StepStatus::Success,
                content: Some("output B".to_string()),
                execution_time_ms: 0,
                error: None,
            },
        );

        let ctx = assemble_context(&step, &outputs);
        assert!(ctx.contains("output A"));
        assert!(ctx.contains("output B"));
        assert!(ctx.contains("step 'a'"));
        assert!(ctx.contains("step 'b'"));
    }

    // ── load_workflow ──────────────────────────────────────────────────

    #[test]
    fn load_workflow_not_found() {
        let result = load_workflow(Path::new("nonexistent/workflow.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn load_workflow_parses_real_file() {
        let path = Path::new("config/workflows").join("coding-quick-dev.yaml");
        if path.exists() {
            let wf = load_workflow(&path).unwrap();
            assert_eq!(wf.name, "coding-quick-dev");
            assert!(!wf.steps.is_empty());
        }
    }

    // ── execute_workflow ───────────────────────────────────────────────

    #[test]
    fn execute_workflow_unknown_id_returns_error() {
        let result = execute_workflow("nonexistent-workflow", "test");
        assert!(result.is_err());
    }

    // ── parse_claude_response ─────────────────────────────────────────

    #[test]
    fn parse_claude_response_success_with_session_id() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "step_id": "s1",
                "status": "success",
                "content": "{\"result\":\"hello world\",\"session_id\":\"abc-123\",\"total_cost_usd\":0.01}",
                "execution_time_ms": 100,
            }
        });
        let (output, session_id) = parse_claude_response("s1", response, 100).unwrap();
        assert_eq!(output.status, StepStatus::Success);
        assert_eq!(output.content.as_deref(), Some("hello world"));
        assert_eq!(session_id.as_deref(), Some("abc-123"));
    }

    #[test]
    fn parse_claude_response_plain_content() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "step_id": "s1",
                "status": "success",
                "content": "plain text output",
                "execution_time_ms": 50,
            }
        });
        let (output, session_id) = parse_claude_response("s1", response, 50).unwrap();
        assert_eq!(output.status, StepStatus::Success);
        assert_eq!(output.content.as_deref(), Some("plain text output"));
        assert!(session_id.is_none());
    }

    #[test]
    fn parse_claude_response_error() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32603,
                "message": "something broke",
            }
        });
        let (output, session_id) = parse_claude_response("s1", response, 50).unwrap();
        assert_eq!(output.status, StepStatus::Failed);
        assert!(output.error.as_deref().unwrap().contains("something broke"));
        assert!(session_id.is_none());
    }

    // ── extract_agent_name ─────────────────────────────────────────────

    #[test]
    fn extract_agent_name_from_system_prompt() {
        let config = StepConfigDef {
            system_prompt: Some("You are bmad/architect. Design the system.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        };
        assert_eq!(extract_agent_name(&config), "bmad/architect");
    }

    #[test]
    fn extract_agent_name_default_when_missing() {
        let config = StepConfigDef {
            system_prompt: Some("You are a developer.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        };
        assert_eq!(extract_agent_name(&config), "bmad/architect");
    }

    #[test]
    fn extract_agent_name_with_punctuation() {
        let config = StepConfigDef {
            system_prompt: Some("You are bmad/qa. Review the code.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        };
        assert_eq!(extract_agent_name(&config), "bmad/qa");
    }

    // ── extract_verdict (quality gate) ─────────────────────────────────

    #[test]
    fn extract_verdict_approve() {
        let content = "Review looks good.\nverdict: approve\n";
        assert_eq!(extract_verdict(content), Some("approve".to_string()));
    }

    #[test]
    fn extract_verdict_request_changes() {
        let content = "Found issues:\n- missing tests\nverdict: request-changes";
        assert_eq!(
            extract_verdict(content),
            Some("request-changes".to_string())
        );
    }

    #[test]
    fn extract_verdict_reject() {
        let content = "Critical security flaw.\nverdict: reject";
        assert_eq!(extract_verdict(content), Some("reject".to_string()));
    }

    #[test]
    fn extract_verdict_case_insensitive() {
        let content = "Verdict: Approve";
        assert_eq!(extract_verdict(content), Some("approve".to_string()));
    }

    #[test]
    fn extract_verdict_not_found() {
        let content = "Review complete. All looks good.";
        assert_eq!(extract_verdict(content), None);
    }

    #[test]
    fn extract_verdict_invalid_value_ignored() {
        let content = "verdict: maybe";
        assert_eq!(extract_verdict(content), None);
    }

    // ── is_review_step ─────────────────────────────────────────────────

    #[test]
    fn is_review_step_true_for_review() {
        let mut step = make_step("qa", &[]);
        step.step_type = "agent".to_string();
        step.config = Some(StepConfigDef {
            system_prompt: Some("You are bmad/qa. Review the implementation.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        });
        assert!(is_review_step(&step));
    }

    #[test]
    fn is_review_step_false_for_implement() {
        let mut step = make_step("impl", &[]);
        step.step_type = "agent".to_string();
        step.config = Some(StepConfigDef {
            system_prompt: Some("You are a developer. Implement the feature.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        });
        assert!(!is_review_step(&step));
    }

    #[test]
    fn is_review_step_false_for_function() {
        let step = make_step("git_commit", &[]);
        assert!(!is_review_step(&step));
    }

    // ── build_retry_context ──────────────────────────────────────────

    #[test]
    fn build_retry_context_attempt_2() {
        let ctx = build_retry_context(2, "exit code 1: test failed", "FAILED test_add", None);
        assert!(ctx.contains("Retry Attempt 2"));
        assert!(ctx.contains("Fix the issues while preserving"));
        assert!(ctx.contains("exit code 1"));
        assert!(ctx.contains("FAILED test_add"));
    }

    #[test]
    fn build_retry_context_attempt_3_with_history() {
        let history = vec![
            serde_json::json!({"attempt": 1, "status": "failed", "failure_summary": "test_add failed"}),
            serde_json::json!({"attempt": 2, "status": "failed", "failure_summary": "test_add still failing"}),
        ];
        let ctx = build_retry_context(3, "exit code 1", "FAILED", Some(&history));
        assert!(ctx.contains("Retry Attempt 3"));
        assert!(ctx.contains("Failure Pattern Analysis"));
        assert!(ctx.contains("different approach"));
    }

    #[test]
    fn build_retry_context_truncates_long_output() {
        let long_output = "x".repeat(5000);
        let ctx = build_retry_context(2, &long_output, &long_output, None);
        // Should not contain the full 5000 chars — truncated to 2000
        assert!(ctx.len() < 10000);
    }

    // ── RetryConfig deserialization ──────────────────────────────────

    #[test]
    fn retry_config_deserializes_from_yaml() {
        let yaml = r#"
            system_prompt: "test"
            retry:
              max_attempts: 3
              on_failure_of: "run_tests"
        "#;
        let config: StepConfigDef = serde_yaml::from_str(yaml).unwrap();
        let retry = config.retry.unwrap();
        assert_eq!(retry.max_attempts, 3);
        assert_eq!(retry.on_failure_of, "run_tests");
    }

    #[test]
    fn retry_config_absent_is_none() {
        let yaml = r#"
            system_prompt: "test"
        "#;
        let config: StepConfigDef = serde_yaml::from_str(yaml).unwrap();
        assert!(config.retry.is_none());
    }

    // ── extract_pr_fields (Story 9.2) ────────────────────────────────

    #[test]
    fn extract_pr_fields_parses_title_and_body() {
        let output = "feat: Add user authentication\n\n## Summary\nAdded login and registration.\n\n## Changes\n- auth.rs\n- login.rs";
        let (title, body) = extract_pr_fields(output);
        assert_eq!(title, "feat: Add user authentication");
        assert!(body.contains("## Summary"));
        assert!(body.contains("auth.rs"));
    }

    #[test]
    fn extract_pr_fields_single_line() {
        let output = "fix: resolve crash on startup";
        let (title, body) = extract_pr_fields(output);
        assert_eq!(title, "fix: resolve crash on startup");
        assert!(body.is_empty());
    }

    #[test]
    fn extract_pr_fields_truncates_long_title() {
        let long_title = "feat: ".to_string() + &"x".repeat(100);
        let output = format!("{}\n\nbody here", long_title);
        let (title, _body) = extract_pr_fields(&output);
        assert!(title.len() <= 72);
    }

    // ── InjectionQuery construction (Story 21-1) ─────────────────────

    /// Helper: construct InjectionQuery the same way execute_bmad_agent_step does.
    fn build_bmad_injection_query(
        config: &StepConfigDef,
        template_vars: &HashMap<String, String>,
    ) -> InjectionQuery {
        let agent_name = extract_agent_name(config);
        let mut query = InjectionQuery::new();
        if agent_name.starts_with("bmad/") {
            query.agent_name = Some(agent_name.clone());
        }
        query.workflow_name = template_vars.get("workflow_id").cloned();
        query.step_type = Some("agent".to_string());
        query
    }

    /// Helper: construct InjectionQuery the same way execute_direct_agent_step does.
    fn build_direct_injection_query(template_vars: &HashMap<String, String>) -> InjectionQuery {
        let mut query = InjectionQuery::new();
        query.workflow_name = template_vars.get("workflow_id").cloned();
        query.step_type = Some("agent".to_string());
        query
    }

    #[test]
    fn test_injection_query_bmad_agent_step() {
        let config = StepConfigDef {
            system_prompt: Some("You are bmad/architect. Design the system.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        };
        let mut template_vars = HashMap::new();
        template_vars.insert("workflow_id".to_string(), "coding-feature-dev".to_string());

        let query = build_bmad_injection_query(&config, &template_vars);
        assert_eq!(query.agent_name.as_deref(), Some("bmad/architect"));
        assert_eq!(query.workflow_name.as_deref(), Some("coding-feature-dev"));
        assert_eq!(query.step_type.as_deref(), Some("agent"));
    }

    #[test]
    fn test_injection_query_non_bmad_step() {
        let mut template_vars = HashMap::new();
        template_vars.insert("workflow_id".to_string(), "coding-quick-dev".to_string());

        let query = build_direct_injection_query(&template_vars);
        assert!(query.agent_name.is_none());
        assert_eq!(query.workflow_name.as_deref(), Some("coding-quick-dev"));
        assert_eq!(query.step_type.as_deref(), Some("agent"));
    }

    #[test]
    fn test_injection_query_missing_workflow_name() {
        let config = StepConfigDef {
            system_prompt: Some("You are bmad/dev. Write code.".to_string()),
            user_prompt_template: None,
            model_tier: None,
            max_tokens: None,
            context_from: None,
            timeout_seconds: None,
            command: None,
            retry: None,
            working_dir: None,
        };
        let template_vars = HashMap::new(); // empty — no workflow_id

        let query = build_bmad_injection_query(&config, &template_vars);
        assert_eq!(query.agent_name.as_deref(), Some("bmad/dev"));
        assert!(query.workflow_name.is_none());
        assert_eq!(query.step_type.as_deref(), Some("agent"));
    }
}
