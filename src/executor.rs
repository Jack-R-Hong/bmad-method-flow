use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const PLUGINS_DIR: &str = "config/plugins";
const WORKFLOWS_DIR: &str = "config/workflows";

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

/// Execute a workflow by ID. Called from `pack::execute_action()`.
pub fn execute_workflow(
    workflow_id: &str,
    user_input: &str,
) -> Result<serde_json::Value, WitPluginError> {
    let start = Instant::now();

    // 1. Load and parse workflow YAML
    let workflow_path = Path::new(WORKFLOWS_DIR).join(format!("{}.yaml", workflow_id));
    let workflow = load_workflow(&workflow_path)?;

    // 2. Check required plugins
    check_required_plugins(&workflow)?;

    // 3. Topological sort
    let execution_order = topological_sort(&workflow.steps)?;

    // 4. Execute steps in DAG order
    let mut outputs: HashMap<String, StepOutput> = HashMap::new();
    let mut template_vars: HashMap<String, String> = HashMap::new();
    template_vars.insert("input".to_string(), user_input.to_string());

    let mut failed_required = false;

    for step_id in &execution_order {
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
            continue;
        }

        // Check if all dependencies are satisfied
        if !dependencies_satisfied(step, &outputs, &workflow.steps) {
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
            continue;
        }

        eprintln!(
            "[workflow] step {}/{}: {} ({})",
            outputs.len() + 1,
            workflow.steps.len(),
            step.id,
            step.step_type
        );

        let result = execute_step(step, &outputs, &template_vars);

        match result {
            Ok(output) => {
                // Extract branch_name from agent output if present
                if let Some(content) = &output.content {
                    if let Some(branch) = extract_branch_name(content) {
                        template_vars.insert("branch_name".to_string(), branch);
                    }
                }

                if output.status != StepStatus::Success && !step.optional {
                    failed_required = true;
                }

                eprintln!(
                    "[workflow]   → {:?} ({}ms)",
                    output.status, output.execution_time_ms
                );
                outputs.insert(step.id.clone(), output);
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
        WitPluginError::not_found(format!(
            "workflow not found: {}: {}",
            path.display(),
            e
        ))
    })?;

    serde_yaml::from_str(&content).map_err(|e| {
        WitPluginError::invalid_input(format!("invalid workflow YAML: {}", e))
    })
}

// ── Plugin availability ────────────────────────────────────────────────────

fn check_required_plugins(workflow: &WorkflowDef) -> Result<(), WitPluginError> {
    if let Some(requires) = &workflow.requires {
        for req in requires {
            let plugin_path = Path::new(PLUGINS_DIR).join(&req.plugin);
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

fn topological_sort(steps: &[StepDef]) -> Result<Vec<String>, WitPluginError> {
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

    // Seed queue with zero-degree nodes, sorted for determinism
    let mut queue: VecDeque<&str> = {
        let mut roots: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        roots.sort();
        roots.into_iter().collect()
    };

    let mut result = Vec::new();

    while let Some(node) = queue.pop_front() {
        result.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            let mut next_ready: Vec<&str> = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next_ready.push(neighbor);
                    }
                }
            }
            next_ready.sort();
            queue.extend(next_ready);
        }
    }

    if result.len() != steps.len() {
        return Err(WitPluginError::invalid_input(
            "workflow DAG contains a cycle",
        ));
    }

    Ok(result)
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

fn execute_step(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    template_vars: &HashMap<String, String>,
) -> Result<StepOutput, WitPluginError> {
    let timeout_secs = step
        .config
        .as_ref()
        .and_then(|c| c.timeout_seconds)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    match step.step_type.as_str() {
        "agent" => execute_agent_step(step, outputs, template_vars, timeout_secs),
        "function" => execute_function_step(step, template_vars, timeout_secs),
        other => Err(WitPluginError::invalid_input(format!(
            "step '{}': unknown step type '{}'",
            step.id, other
        ))),
    }
}

// ── Agent step execution (JSON-RPC over stdio) ─────────────────────────────

fn execute_agent_step(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    template_vars: &HashMap<String, String>,
    timeout_secs: u64,
) -> Result<StepOutput, WitPluginError> {
    let start = Instant::now();

    let executor = step.executor.as_deref().ok_or_else(|| {
        WitPluginError::invalid_input(format!(
            "step '{}': agent step requires 'executor'",
            step.id
        ))
    })?;

    let config = step.config.as_ref().ok_or_else(|| {
        WitPluginError::invalid_input(format!(
            "step '{}': agent step requires 'config'",
            step.id
        ))
    })?;

    let plugin_binary = Path::new(PLUGINS_DIR).join(executor);
    if !plugin_binary.exists() {
        return Err(WitPluginError::not_found(format!(
            "step '{}': executor '{}' not found at {}",
            step.id,
            executor,
            plugin_binary.display()
        )));
    }

    // Build the prompt with context injection
    let context = assemble_context(step, outputs);
    let user_prompt = config
        .user_prompt_template
        .as_deref()
        .map(|t| substitute_templates(t, template_vars))
        .unwrap_or_else(|| {
            template_vars
                .get("input")
                .cloned()
                .unwrap_or_default()
        });

    let full_prompt = if context.is_empty() {
        user_prompt
    } else {
        format!(
            "{}\n\n## Context from prior steps:\n{}",
            user_prompt, context
        )
    };

    // Build JSON-RPC request
    let (task_input, step_config) = build_rpc_params(step, executor, &full_prompt, config);

    let rpc_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "step-executor.execute",
        "params": {
            "task": task_input,
            "config": step_config,
        }
    });

    let response = spawn_plugin_rpc(&plugin_binary, &rpc_request, timeout_secs)?;
    let elapsed = start.elapsed().as_millis() as u64;

    parse_step_response(&step.id, response, elapsed)
}

fn build_rpc_params(
    step: &StepDef,
    executor: &str,
    prompt: &str,
    config: &StepConfigDef,
) -> (serde_json::Value, serde_json::Value) {
    let task_input = match executor {
        "bmad-method" => {
            // Extract agent role from system_prompt (e.g. "bmad/architect")
            let agent = config
                .system_prompt
                .as_deref()
                .and_then(|sp| {
                    sp.split_whitespace()
                        .find(|w| w.starts_with("bmad/"))
                        .map(|w| w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '-').to_string())
                })
                .unwrap_or_else(|| "bmad/architect".to_string());

            serde_json::json!({
                "task_id": format!("wf-{}", step.id),
                "description": prompt,
                "input": {
                    "agent": agent,
                    "prompt": prompt,
                },
            })
        }
        _ => {
            serde_json::json!({
                "task_id": format!("wf-{}", step.id),
                "description": prompt,
                "input": {
                    "prompt": prompt,
                },
            })
        }
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

    let step_config = serde_json::json!({
        "step_id": step.id,
        "step_type": step.step_type,
        "timeout_secs": config.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECS),
        "parameters": parameters,
    });

    (task_input, step_config)
}

// ── Function step execution (direct command) ───────────────────────────────

fn execute_function_step(
    step: &StepDef,
    template_vars: &HashMap<String, String>,
    timeout_secs: u64,
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

    // Resolve program path: check config/plugins/ first, then PATH
    let program = if let Some(executor) = &step.executor {
        let plugin_path = Path::new(PLUGINS_DIR).join(executor);
        if plugin_path.exists() {
            plugin_path.to_string_lossy().to_string()
        } else {
            resolved_cmd[0].clone()
        }
    } else {
        resolved_cmd[0].clone()
    };

    let child = Command::new(&program)
        .args(&resolved_cmd[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
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
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            WitPluginError::internal("failed to open stdin for plugin")
        })?;
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
    let stdout = child.stdout.take().ok_or_else(|| {
        WitPluginError::internal("failed to open stdout for plugin")
    })?;
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

    response.ok_or_else(|| {
        WitPluginError::internal("plugin returned no JSON-RPC response")
    })
}

fn parse_step_response(
    step_id: &str,
    response: serde_json::Value,
    elapsed_ms: u64,
) -> Result<StepOutput, WitPluginError> {
    // Check for JSON-RPC error
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Ok(StepOutput {
            step_id: step_id.to_string(),
            status: StepStatus::Failed,
            content: None,
            execution_time_ms: elapsed_ms,
            error: Some(msg.to_string()),
        });
    }

    let result = response
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let status_str = result
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let content = result
        .get("content")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    let status = if status_str == "success" {
        StepStatus::Success
    } else {
        StepStatus::Failed
    };

    Ok(StepOutput {
        step_id: step_id.to_string(),
        status: status.clone(),
        content,
        execution_time_ms: elapsed_ms,
        error: if status == StepStatus::Failed {
            Some(format!("step returned status: {}", status_str))
        } else {
            None
        },
    })
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
        assert_eq!(
            substitute_templates("no vars here", &vars),
            "no vars here"
        );
    }

    #[test]
    fn substitute_templates_unresolved_var_left_as_is() {
        let vars = HashMap::new();
        assert_eq!(
            substitute_templates("{{unknown}}", &vars),
            "{{unknown}}"
        );
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
        assert_eq!(
            extract_branch_name(content),
            Some("my-feature".to_string())
        );
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
        let path = Path::new(WORKFLOWS_DIR).join("coding-quick-dev.yaml");
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

    // ── parse_step_response ────────────────────────────────────────────

    #[test]
    fn parse_response_success() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "step_id": "s1",
                "status": "success",
                "content": "hello",
                "execution_time_ms": 100,
            }
        });
        let output = parse_step_response("s1", response, 100).unwrap();
        assert_eq!(output.status, StepStatus::Success);
        assert_eq!(output.content.as_deref(), Some("hello"));
    }

    #[test]
    fn parse_response_error() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32603,
                "message": "something broke",
            }
        });
        let output = parse_step_response("s1", response, 50).unwrap();
        assert_eq!(output.status, StepStatus::Failed);
        assert!(output.error.as_deref().unwrap().contains("something broke"));
    }

    // ── build_rpc_params ───────────────────────────────────────────────

    #[test]
    fn build_rpc_params_bmad_extracts_agent() {
        let step = StepDef {
            id: "test".to_string(),
            step_type: "agent".to_string(),
            executor: Some("bmad-method".to_string()),
            depends_on: vec![],
            optional: false,
            config: Some(StepConfigDef {
                system_prompt: Some(
                    "You are bmad/architect. Design the system.".to_string(),
                ),
                user_prompt_template: None,
                model_tier: Some("balanced".to_string()),
                max_tokens: Some(4096),
                context_from: None,
                timeout_seconds: None,
                command: None,
            }),
        };
        let config = step.config.as_ref().unwrap();
        let (task, cfg) = build_rpc_params(&step, "bmad-method", "test prompt", config);

        assert_eq!(task["input"]["agent"], "bmad/architect");
        assert_eq!(cfg["parameters"]["model_tier"], "balanced");
        assert_eq!(cfg["parameters"]["max_tokens"], 4096);
    }

    #[test]
    fn build_rpc_params_claude_code() {
        let step = StepDef {
            id: "impl".to_string(),
            step_type: "agent".to_string(),
            executor: Some("provider-claude-code".to_string()),
            depends_on: vec![],
            optional: false,
            config: Some(StepConfigDef {
                system_prompt: Some("You are a developer.".to_string()),
                user_prompt_template: None,
                model_tier: None,
                max_tokens: None,
                context_from: None,
                timeout_seconds: None,
                command: None,
            }),
        };
        let config = step.config.as_ref().unwrap();
        let (task, _cfg) = build_rpc_params(&step, "provider-claude-code", "do it", config);

        assert_eq!(task["input"]["prompt"], "do it");
        // No agent field for non-bmad executors
        assert!(task["input"].get("agent").is_none());
    }
}
