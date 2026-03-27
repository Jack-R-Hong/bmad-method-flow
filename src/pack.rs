use crate::util::is_executable;
use crate::validator;
use crate::workspace::WorkspaceConfig;
use pulse_plugin_sdk::error::WitPluginError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CodingPackInput {
    pub action: String,
    /// Optional: target plugin name for plugin-specific actions
    #[serde(default)]
    pub target: Option<String>,
    /// Workflow ID for execute-workflow action
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// User input / task description for execute-workflow action
    #[serde(default)]
    pub input: Option<String>,
    /// Data endpoint path for data-query action (set by Pulse proxy)
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Mutation payload for data-mutate action
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    /// Optional workspace root directory override.
    /// If not set, falls back to PULSE_WORKSPACE_DIR env var, then current directory.
    #[serde(default)]
    pub workspace_dir: Option<String>,
}

/// Execute a pack-level action.
pub fn execute_action(input: &CodingPackInput) -> Result<String, WitPluginError> {
    let config = WorkspaceConfig::resolve(input.workspace_dir.as_deref());

    match input.action.as_str() {
        "validate-pack" => to_json_string(validate_pack_value(&config)),
        "validate-workflows" => to_json_string(validate_workflows_value(&config)),
        "list-workflows" => to_json_string(list_workflows_value(&config)),
        "list-plugins" => to_json_string(list_plugins_value(&config)),
        "status" => to_json_string(pack_status_value(&config)),
        "execute-workflow" => {
            let workflow_id = input.workflow_id.as_deref().ok_or_else(|| {
                WitPluginError::invalid_input("execute-workflow requires 'workflow_id'")
            })?;
            if !config.is_workflow_enabled(workflow_id) {
                return Err(WitPluginError::not_found(format!(
                    "Workflow '{}' is disabled in this workspace",
                    workflow_id
                )));
            }
            let user_input = input.input.as_deref().unwrap_or("");
            to_json_string(crate::executor::execute_workflow_with_config(
                workflow_id,
                user_input,
                &config,
            ))
        }
        "data-query" => {
            let endpoint = input.endpoint.as_deref().unwrap_or("");
            execute_data_query(endpoint, &config)
        }
        "data-mutate" => {
            let endpoint = input.endpoint.as_deref().unwrap_or("");
            let payload = input.payload.clone().unwrap_or(serde_json::Value::Null);
            execute_data_mutate(endpoint, &payload, &config)
        }
        other => Err(WitPluginError::not_found(format!(
            "Unknown action: '{}'. Available: validate-pack, validate-workflows, list-workflows, list-plugins, status, execute-workflow, data-query, data-mutate",
            other
        ))),
    }
}

fn to_json_string(
    result: Result<serde_json::Value, WitPluginError>,
) -> Result<String, WitPluginError> {
    result.map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
}

fn validate_pack_value(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let mut issues = Vec::new();
    let mut ok_count = 0;

    // Check required plugins
    let required_plugins = ["bmad-method", "provider-claude-code"];
    let optional_plugins = ["plugin-git-worktree", "plugin-memory"];

    for plugin in &required_plugins {
        let path = config.plugins_dir.join(plugin);
        if path.exists() {
            ok_count += 1;
        } else {
            issues.push(format!("MISSING required plugin: {}", plugin));
        }
    }

    for plugin in &optional_plugins {
        let path = config.plugins_dir.join(plugin);
        if path.exists() {
            ok_count += 1;
        } else {
            issues.push(format!(
                "MISSING optional plugin: {} (non-blocking)",
                plugin
            ));
        }
    }

    // Check workflow files
    let workflow_count = if config.workflows_dir.exists() {
        std::fs::read_dir(&config.workflows_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("yaml"))
                    .count()
            })
            .unwrap_or(0)
    } else {
        issues.push(format!(
            "MISSING workflows directory: {}",
            config.workflows_dir.display()
        ));
        0
    };

    Ok(serde_json::json!({
        "valid": issues.iter().all(|i| i.contains("optional") || i.contains("non-blocking")),
        "plugins_ok": ok_count,
        "workflows_found": workflow_count,
        "issues": issues,
    }))
}

fn validate_workflows_value(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    if !config.workflows_dir.exists() {
        return Ok(serde_json::json!({
            "valid": false,
            "results": [],
            "issues": [format!("workflows directory not found: {}", config.workflows_dir.display())],
        }));
    }

    let mut results = Vec::new();
    let mut all_valid = true;

    let mut entries: Vec<_> = std::fs::read_dir(&config.workflows_dir)
        .map_err(|e| WitPluginError::internal(format!("cannot read workflows dir: {}", e)))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("yaml"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        match validator::validate_workflow_file(&path, &config.plugins_dir) {
            Ok(result) => {
                if !result.valid {
                    all_valid = false;
                }
                results.push(serde_json::json!({
                    "file": result.file,
                    "valid": result.valid,
                    "issues": result.issues,
                }));
            }
            Err(e) => {
                all_valid = false;
                results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "valid": false,
                    "issues": [e],
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "valid": all_valid,
        "count": results.len(),
        "results": results,
    }))
}

fn list_workflows_value(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let mut workflows = Vec::new();

    if config.workflows_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&config.workflows_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        if config.is_workflow_enabled(name) {
                            workflows.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    workflows.sort();
    Ok(serde_json::json!({
        "workflows": workflows,
        "count": workflows.len(),
    }))
}

fn list_plugins_value(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let mut plugins = Vec::new();

    if config.plugins_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&config.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if !name.starts_with('.') {
                        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        plugins.push(serde_json::json!({
                            "name": name,
                            "size_bytes": size,
                            "executable": is_executable(&path),
                        }));
                    }
                }
            }
        }
    }

    Ok(serde_json::json!({
        "plugins": plugins,
        "count": plugins.len(),
    }))
}

fn pack_status_value(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    Ok(serde_json::json!({
        "validation": validate_pack_value(config)?,
        "workflows": list_workflows_value(config)?,
        "plugins": list_plugins_value(config)?,
    }))
}

/// Handle data-query requests from dashboard proxy.
/// Routes endpoint paths to internal data functions.
fn execute_data_query(endpoint: &str, config: &WorkspaceConfig) -> Result<String, WitPluginError> {
    let endpoint = endpoint.trim_start_matches('/');
    let result = match endpoint {
        "status" => pack_status_value(config)?,
        "workflows/list" => list_workflows_detail_value(config)?,
        "agents/list" => list_agents_value()?,
        "board/data" => crate::board::get_board_data(config)?,
        "board/epics/list" => crate::board::get_epics_list(config)?,
        "board/assignments/list" => {
            crate::board_store::get_assignments_list_from_store(config)?
        }
        ep if ep.starts_with("board/assignments/") => {
            let id = ep.strip_prefix("board/assignments/").unwrap_or("");
            crate::board_store::get_assignment_detail_from_store(id, config)?
        }
        "board/filters" => crate::board::get_filter_options(config)?,
        "board/summary" => crate::board::get_board_summary(config)?,
        ep if ep.starts_with("board/epics/") => {
            let id = ep.strip_prefix("board/epics/").unwrap_or("");
            crate::board::get_epic_detail(id, config)?
        }
        ep if ep.starts_with("board/stories/") => {
            let id = ep.strip_prefix("board/stories/").unwrap_or("");
            crate::board::get_story_detail(id, config)?
        }
        ep if ep.starts_with("workflows/") => {
            let id = ep.strip_prefix("workflows/").unwrap_or("");
            get_workflow_detail_value(id, config)?
        }
        _ => {
            return Err(WitPluginError::not_found(format!(
                "Unknown data endpoint: '{}'. Available: status, workflows/list, agents/list, workflows/{{id}}, board/data, board/filters, board/summary, board/epics/{{id}}, board/stories/{{id}}",
                endpoint
            )));
        }
    };
    serde_json::to_string_pretty(&result)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

/// Detailed workflow list for dashboard table view.
fn list_workflows_detail_value(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let mut workflows = Vec::new();

    if config.workflows_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&config.workflows_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                    let id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");

                    if !config.is_workflow_enabled(id) {
                        continue;
                    }

                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(wf) = serde_yaml::from_str::<serde_json::Value>(&content) {
                            let category = if id.starts_with("bootstrap") {
                                "bootstrap"
                            } else {
                                "coding"
                            };
                            workflows.push(serde_json::json!({
                                "id": id,
                                "description": wf.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                                "category": category,
                                "step_count": wf.get("steps").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0),
                                "requires": wf.get("requires").and_then(|r| r.as_array()).map(|arr| {
                                    arr.iter().filter_map(|r| r.get("plugin").and_then(|p| p.as_str())).collect::<Vec<_>>().join(", ")
                                }).unwrap_or_default(),
                            }));
                        }
                    }
                }
            }
        }
    }

    workflows.sort_by(|a, b| {
        let a_id = a["id"].as_str().unwrap_or("");
        let b_id = b["id"].as_str().unwrap_or("");
        a_id.cmp(b_id)
    });

    Ok(serde_json::json!(workflows))
}

/// BMAD agent list for dashboard table view.
fn list_agents_value() -> Result<serde_json::Value, WitPluginError> {
    let agents = serde_json::json!([
        {"id": "bmad/architect", "name": "Winston", "role": "Architect", "assigned_workflows": "coding-feature-dev, coding-story-dev"},
        {"id": "bmad/dev", "name": "Amelia", "role": "Developer", "assigned_workflows": "coding-feature-dev, coding-story-dev, coding-bug-fix"},
        {"id": "bmad/pm", "name": "John", "role": "Product Manager", "assigned_workflows": "coding-feature-dev"},
        {"id": "bmad/qa", "name": "Quinn", "role": "QA Engineer", "assigned_workflows": "coding-feature-dev, coding-story-dev, coding-review"},
        {"id": "bmad/sm", "name": "Bob", "role": "Scrum Master", "assigned_workflows": "coding-story-dev"},
        {"id": "bmad/quick-flow", "name": "Barry", "role": "Quick Flow Solo Dev", "assigned_workflows": "coding-quick-dev"},
        {"id": "bmad/analyst", "name": "Mary", "role": "Analyst", "assigned_workflows": ""},
        {"id": "bmad/ux-designer", "name": "Sally", "role": "UX Designer", "assigned_workflows": ""},
        {"id": "bmad/tech-writer", "name": "Paige", "role": "Tech Writer", "assigned_workflows": ""},
    ]);
    Ok(agents)
}

/// Single workflow detail for dashboard detail view.
fn get_workflow_detail_value(
    workflow_id: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let path = config.workflows_dir.join(format!("{}.yaml", workflow_id));
    if !path.exists() {
        return Err(WitPluginError::not_found(format!(
            "Workflow '{}' not found",
            workflow_id
        )));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| WitPluginError::internal(format!("Cannot read workflow: {e}")))?;
    let wf: serde_json::Value = serde_yaml::from_str(&content)
        .map_err(|e| WitPluginError::internal(format!("Invalid YAML: {e}")))?;

    let steps = wf
        .get("steps")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let step_pipeline: Vec<String> = steps
        .iter()
        .filter_map(|s| {
            s.get("id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let category = if workflow_id.starts_with("bootstrap") {
        "bootstrap"
    } else {
        "coding"
    };

    Ok(serde_json::json!({
        "id": workflow_id,
        "description": wf.get("description").and_then(|d| d.as_str()).unwrap_or(""),
        "category": category,
        "step_count": steps.len(),
        "step_pipeline": step_pipeline.join(" → "),
        "requires": wf.get("requires").and_then(|r| r.as_array()).map(|arr| {
            arr.iter().filter_map(|r| r.get("plugin").and_then(|p| p.as_str())).collect::<Vec<_>>().join(", ")
        }).unwrap_or_default(),
    }))
}

/// Handle data-mutate requests for board CRUD operations.
fn execute_data_mutate(
    endpoint: &str,
    payload: &serde_json::Value,
    config: &WorkspaceConfig,
) -> Result<String, WitPluginError> {
    let endpoint = endpoint.trim_start_matches('/');
    let result = match endpoint {
        "board/sync" => {
            let store = crate::board_store::sync_from_artifacts(config)?;
            serde_json::to_value(&store)
                .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?
        }
        ep if ep.starts_with("board/status/") => {
            let item_id = ep.strip_prefix("board/status/").unwrap_or("");
            let new_status = payload
                .get("status")
                .and_then(|s| s.as_str())
                .ok_or_else(|| WitPluginError::invalid_input("'status' field required in payload"))?;
            crate::board_store::update_item_status(&config.base_dir, item_id, new_status)?
        }
        "board/epics" => crate::board_store::create_epic(&config.base_dir, payload)?,
        ep if ep.starts_with("board/epics/") => {
            let epic_id = ep.strip_prefix("board/epics/").unwrap_or("");
            crate::board_store::update_epic(&config.base_dir, epic_id, payload)?
        }
        "board/stories" => crate::board_store::create_story(&config.base_dir, payload)?,
        ep if ep.starts_with("board/stories/") => {
            let story_id = ep.strip_prefix("board/stories/").unwrap_or("");
            crate::board_store::update_story(&config.base_dir, story_id, payload)?
        }
        _ => {
            return Err(WitPluginError::not_found(format!(
                "Unknown mutation endpoint: '{}'. Available: board/sync, board/status/{{id}}, board/epics, board/epics/{{id}}, board/stories, board/stories/{{id}}",
                endpoint
            )));
        }
    };
    serde_json::to_string_pretty(&result)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_input(action: &str) -> CodingPackInput {
        CodingPackInput {
            action: action.to_string(),
            target: None,
            workflow_id: None,
            input: None,
            endpoint: None,
            payload: None,
            workspace_dir: None,
        }
    }

    #[test]
    fn validate_pack_returns_valid_json() {
        let input = test_input("validate-pack");
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("plugins_ok").is_some());
        assert!(parsed.get("workflows_found").is_some());
    }

    #[test]
    fn validate_workflows_returns_valid_json() {
        let input = test_input("validate-workflows");
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("count").is_some());
        assert!(parsed.get("results").is_some());
    }

    #[test]
    fn list_workflows_returns_valid_json() {
        let input = test_input("list-workflows");
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("workflows").is_some());
        assert!(parsed.get("count").is_some());
    }

    #[test]
    fn list_plugins_returns_valid_json() {
        let input = test_input("list-plugins");
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("plugins").is_some());
    }

    #[test]
    fn unknown_action_returns_not_found() {
        let input = test_input("does-not-exist");
        let err = execute_action(&input).unwrap_err();
        assert_eq!(err.code, "not_found");
    }
}
