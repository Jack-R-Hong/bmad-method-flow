pub mod executor;
pub mod pack;
pub mod test_parser;
pub mod util;
pub mod validator;

use pulse_plugin_sdk::error::WitPluginError;
use pulse_plugin_sdk::wit_traits::{DashboardExtensionPlugin, PluginLifecycle, StepExecutorPlugin};
use pulse_plugin_sdk::wit_types::{
    PluginDependency, PluginInfo, StepConfig, StepResult, TaskInput,
};
use tracing::info;

use pack::CodingPackInput;
use util::is_executable;

/// Meta-plugin that orchestrates the coding plugin pack.
///
/// Validates that all required sibling plugins are present and healthy,
/// provides workflow validation, and exposes a step executor for pack-level operations.
#[derive(Default)]
pub struct CodingPackPlugin;

impl PluginLifecycle for CodingPackPlugin {
    fn get_info(&self) -> PluginInfo {
        PluginInfo::new("plugin-coding-pack", env!("CARGO_PKG_VERSION"))
            .with_description(
                "Coding pack orchestrator — coordinates bmad-method, provider-claude-code, and git-worktree plugins",
            )
            .with_dependencies(vec![
                PluginDependency {
                    name: "bmad-method".to_string(),
                    version_req: ">=0.1.0".to_string(),
                    optional: false,
                },
                PluginDependency {
                    name: "provider-claude-code".to_string(),
                    version_req: ">=0.1.0".to_string(),
                    optional: false,
                },
                PluginDependency {
                    name: "plugin-git-worktree".to_string(),
                    version_req: ">=0.1.0".to_string(),
                    optional: true,
                },
                PluginDependency {
                    name: "plugin-memory".to_string(),
                    version_req: ">=0.1.0".to_string(),
                    optional: true,
                },
            ])
    }

    fn health_check(&self) -> bool {
        let workflows_dir = std::path::Path::new("config/workflows");
        let plugins_dir = std::path::Path::new("config/plugins");

        let workflows_ok = workflows_dir.exists();
        let plugins_ok = plugins_dir.exists();

        // Verify required plugin binaries exist and are executable
        let required_plugins = ["bmad-method", "provider-claude-code"];
        let mut plugins_healthy = true;
        for plugin_name in &required_plugins {
            let plugin_path = plugins_dir.join(plugin_name);
            if !plugin_path.exists() {
                tracing::warn!(
                    plugin = "plugin-coding-pack",
                    missing = plugin_name,
                    "Required plugin binary not found"
                );
                plugins_healthy = false;
            } else if !is_executable(&plugin_path) {
                tracing::warn!(
                    plugin = "plugin-coding-pack",
                    not_executable = plugin_name,
                    "Plugin binary is not executable"
                );
                plugins_healthy = false;
            }
        }

        let healthy = workflows_ok && plugins_ok && plugins_healthy;

        if healthy {
            info!(
                plugin = "plugin-coding-pack",
                status = "healthy",
                "Coding pack health check passed"
            );
        } else {
            tracing::warn!(
                plugin = "plugin-coding-pack",
                workflows_dir_exists = workflows_ok,
                plugins_dir_exists = plugins_ok,
                plugins_healthy = plugins_healthy,
                "Coding pack health check: issues detected"
            );
        }

        healthy
    }
}

impl StepExecutorPlugin for CodingPackPlugin {
    fn execute(&self, task: TaskInput, config: StepConfig) -> Result<StepResult, WitPluginError> {
        // Respond to capability probe
        if task.task_id == "__probe__" {
            return Ok(StepResult {
                step_id: "__probe__".to_string(),
                status: "probe_ok".to_string(),
                content: None,
                execution_time_ms: 0,
            });
        }

        let input_val = task.input.as_ref().ok_or_else(|| {
            WitPluginError::invalid_input(
                "task input required; send JSON {\"action\": \"validate-pack\"}, {\"action\": \"validate-workflows\"}, or {\"action\": \"list-workflows\"}",
            )
        })?;

        let pack_input: CodingPackInput = serde_json::from_value(input_val.clone())
            .map_err(|e| WitPluginError::invalid_input(format!("invalid input: {e}")))?;

        let start = std::time::Instant::now();
        let result = pack::execute_action(&pack_input)?;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(StepResult {
            step_id: config.step_id,
            status: "success".to_string(),
            content: Some(result),
            execution_time_ms: elapsed,
        })
    }
}

impl DashboardExtensionPlugin for CodingPackPlugin {
    fn get_pages_json(&self) -> String {
        serde_json::json!([
            {
                "id": "overview",
                "title": "Coding Pack",
                "path": "/overview",
                "icon": "package",
                "nav_order": 0,
                "description": "Pack health, workflows, plugins, and AI agents at a glance",
                "layout": {
                    "type": "detail",
                    "sections": [
                        {
                            "id": "health",
                            "title": "Pack Health",
                            "fields": [
                                { "key": "valid", "label": "Valid" },
                                { "key": "plugins_ok", "label": "Plugins OK" },
                                { "key": "workflows_found", "label": "Workflows Found" }
                            ]
                        },
                        {
                            "id": "workflows",
                            "title": "Workflows",
                            "fields": [
                                { "key": "workflow_count", "label": "Total Workflows" },
                                { "key": "workflow_categories", "label": "Categories" }
                            ]
                        },
                        {
                            "id": "plugins",
                            "title": "Installed Plugins",
                            "fields": [
                                { "key": "plugins.count", "label": "Plugin Count" },
                                { "key": "plugins.plugins", "label": "Plugin List" }
                            ]
                        }
                    ],
                    "data_endpoint": "status"
                }
            },
            {
                "id": "workflows",
                "title": "Workflows",
                "path": "/workflows",
                "icon": "git-branch",
                "nav_order": 1,
                "description": "Browse and manage all coding and bootstrap workflows",
                "layout": {
                    "type": "table",
                    "columns": [
                        { "key": "id", "label": "Workflow ID", "sortable": true },
                        { "key": "description", "label": "Description", "sortable": false },
                        { "key": "category", "label": "Category", "sortable": true },
                        { "key": "step_count", "label": "Steps", "sortable": true },
                        { "key": "requires", "label": "Required Plugins", "sortable": false },
                        { "key": "last_run", "label": "Last Run", "sortable": true }
                    ],
                    "data_endpoint": "workflows/list",
                    "row_actions": [
                        { "id": "execute", "label": "Execute", "method": "POST", "endpoint": "workflows/{id}/execute" },
                        { "id": "view", "label": "View Steps", "method": "GET", "endpoint": "workflows/{id}" }
                    ],
                    "bulk_actions": []
                }
            },
            {
                "id": "workflow-detail",
                "title": "Workflow Detail",
                "path": "/workflows/:id",
                "icon": "git-branch",
                "nav_order": 99,
                "description": "Detailed workflow steps and execution history",
                "layout": {
                    "type": "detail",
                    "sections": [
                        {
                            "id": "info",
                            "title": "Workflow Info",
                            "fields": [
                                { "key": "id", "label": "Workflow ID" },
                                { "key": "description", "label": "Description" },
                                { "key": "category", "label": "Category" },
                                { "key": "requires", "label": "Required Plugins" }
                            ]
                        },
                        {
                            "id": "steps",
                            "title": "Pipeline Steps",
                            "fields": [
                                { "key": "step_count", "label": "Total Steps" },
                                { "key": "step_pipeline", "label": "Pipeline" },
                                { "key": "parallel_groups", "label": "Parallel Groups" }
                            ]
                        },
                        {
                            "id": "recent",
                            "title": "Recent Executions",
                            "fields": [
                                { "key": "last_run", "label": "Last Run" },
                                { "key": "total_runs", "label": "Total Runs" },
                                { "key": "success_rate", "label": "Success Rate" }
                            ]
                        }
                    ],
                    "data_endpoint": "workflows/{id}"
                }
            },
            {
                "id": "agents",
                "title": "AI Agents",
                "path": "/agents",
                "icon": "bot",
                "nav_order": 2,
                "description": "BMAD AI team members and their roles",
                "layout": {
                    "type": "table",
                    "columns": [
                        { "key": "id", "label": "Agent ID", "sortable": true },
                        { "key": "name", "label": "Name", "sortable": true },
                        { "key": "role", "label": "Role", "sortable": true },
                        { "key": "assigned_workflows", "label": "Workflows", "sortable": true }
                    ],
                    "data_endpoint": "agents/list",
                    "row_actions": [
                        { "id": "view", "label": "View", "method": "GET", "endpoint": "agents/{id}" }
                    ],
                    "bulk_actions": []
                }
            },
            {
                "id": "status",
                "title": "Pack Status",
                "path": "/status",
                "icon": "activity",
                "nav_order": 3,
                "description": "Pack health, validation results, and plugin status",
                "layout": {
                    "type": "detail",
                    "sections": [
                        {
                            "id": "health",
                            "title": "Pack Health",
                            "fields": [
                                { "key": "valid", "label": "Valid" },
                                { "key": "plugins_ok", "label": "Plugins OK" },
                                { "key": "workflows_found", "label": "Workflows Found" }
                            ]
                        },
                        {
                            "id": "plugins",
                            "title": "Installed Plugins",
                            "fields": [
                                { "key": "plugins.count", "label": "Plugin Count" },
                                { "key": "plugins.plugins", "label": "Plugin List" }
                            ]
                        },
                        {
                            "id": "validation",
                            "title": "Validation",
                            "fields": [
                                { "key": "validation.valid", "label": "Passed" },
                                { "key": "validation.issues", "label": "Issues" }
                            ]
                        }
                    ],
                    "data_endpoint": "status"
                }
            },
            {
                "id": "execute",
                "title": "Execute Workflow",
                "path": "/execute",
                "icon": "play",
                "nav_order": 4,
                "description": "Trigger a workflow execution",
                "layout": {
                    "type": "form",
                    "fields": [
                        {
                            "id": "workflow_id",
                            "label": "Workflow",
                            "field_type": { "select": { "options": [
                                "coding-quick-dev", "coding-feature-dev", "coding-story-dev",
                                "coding-bug-fix", "coding-refactor", "coding-review",
                                "bootstrap-plugin", "bootstrap-rebuild", "bootstrap-cycle"
                            ]}},
                            "required": true
                        },
                        { "id": "input", "label": "Task Description", "field_type": "textarea", "required": true },
                        { "id": "target", "label": "Target Path (for code-review)", "field_type": "text", "required": false }
                    ],
                    "submit_endpoint": "workflows/execute"
                }
            },
            {
                "id": "logs",
                "title": "Execution Logs",
                "path": "/logs",
                "icon": "terminal",
                "nav_order": 5,
                "description": "Real-time execution event stream",
                "layout": {
                    "type": "stream",
                    "event_endpoint": "executions/stream",
                    "event_types": ["execution_start", "step_start", "step_complete", "step_error", "execution_complete"]
                }
            }
        ])
        .to_string()
    }

    fn get_api_routes_json(&self) -> String {
        serde_json::json!([
            {
                "prefix": "/api/v1/plugin-coding-pack",
                "description": "Coding pack status, validation, and workflow management",
                "endpoints": [
                    "GET  /status          — Pack health and validation",
                    "GET  /status/health    — Health badge data",
                    "GET  /workflows/list   — All workflows as table data",
                    "GET  /workflows/{id}   — Workflow detail with steps",
                    "POST /workflows/{id}/execute — Trigger workflow execution",
                    "GET  /agents/list      — BMAD agent roster",
                    "GET  /agents/{id}      — Agent detail",
                    "GET  /executions/stream — SSE execution event stream",
                    "GET  /tasks/{task_id}/workflow-context — Task workflow context",
                    "GET  /tasks/{task_id}/agent-info — Task agent info"
                ]
            }
        ])
        .to_string()
    }

    fn get_display_customizations_json(&self) -> String {
        serde_json::json!([
            {
                "id": "coding-pack-health",
                "title": "Coding Pack",
                "target_view": "workflow",
                "customization": {
                    "type": "badge",
                    "key": "pack_status",
                    "label": "Pack",
                    "color_mapping": {
                        "healthy": "#10b981",
                        "degraded": "#f59e0b",
                        "error": "#ef4444",
                        "default": "#64748b"
                    }
                },
                "data_endpoint": "status/health",
                "render_priority": 10
            },
            {
                "id": "coding-workflow-info",
                "title": "Workflow Details",
                "target_view": "task",
                "customization": {
                    "type": "fields",
                    "fields": [
                        { "key": "workflow_id", "label": "Workflow" },
                        { "key": "step_id", "label": "Current Step" },
                        { "key": "executor", "label": "Executor" },
                        { "key": "model_tier", "label": "Model Tier" }
                    ]
                },
                "data_endpoint": "tasks/{task_id}/workflow-context",
                "render_priority": 20
            },
            {
                "id": "coding-pack-agent",
                "title": "BMAD Agent",
                "target_view": "task",
                "customization": {
                    "type": "badge",
                    "key": "agent_name",
                    "label": "Agent",
                    "color_mapping": {
                        "Winston": "#3b82f6",
                        "Amelia": "#10b981",
                        "John": "#8b5cf6",
                        "Quinn": "#f59e0b",
                        "Bob": "#06b6d4",
                        "Barry": "#ef4444",
                        "Mary": "#ec4899",
                        "Sally": "#14b8a6",
                        "Paige": "#a855f7",
                        "default": "#64748b"
                    }
                },
                "data_endpoint": "tasks/{task_id}/agent-info",
                "render_priority": 30
            }
        ])
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_health_check_returns_true() {
        // Create stub plugin binaries so health_check can find them
        let plugins_dir = std::path::Path::new("config/plugins");
        std::fs::create_dir_all(plugins_dir).unwrap();

        for name in &["bmad-method", "provider-claude-code"] {
            let path = plugins_dir.join(name);
            if !path.exists() {
                std::fs::write(&path, "#!/bin/sh\n").unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
            }
        }

        let plugin = CodingPackPlugin;
        assert!(plugin.health_check());
    }

    #[test]
    fn plugin_info_has_correct_name() {
        let plugin = CodingPackPlugin;
        let info = plugin.get_info();
        assert_eq!(info.name, "plugin-coding-pack");
        assert!(!info.version.is_empty());
    }

    #[test]
    fn plugin_info_declares_dependencies() {
        let plugin = CodingPackPlugin;
        let info = plugin.get_info();
        assert!(info.dependencies.len() >= 2);
        let names: Vec<&str> = info.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"bmad-method"));
        assert!(names.contains(&"provider-claude-code"));
    }

    #[test]
    fn probe_returns_ok() {
        let plugin = CodingPackPlugin;
        let task = TaskInput::new("__probe__", "");
        let config = StepConfig::new("__probe__", "");
        let result = plugin.execute(task, config).unwrap();
        assert_eq!(result.status, "probe_ok");
    }

    #[test]
    fn execute_validate_pack_returns_success() {
        let plugin = CodingPackPlugin;
        let task = TaskInput::new("t1", "validate pack")
            .with_input(serde_json::json!({"action": "validate-pack"}));
        let config = StepConfig::new("s1", "agent");
        let result = plugin.execute(task, config).unwrap();
        assert_eq!(result.status, "success");
        assert!(result.content.is_some());
    }

    #[test]
    fn execute_list_workflows_returns_success() {
        let plugin = CodingPackPlugin;
        let task = TaskInput::new("t1", "list workflows")
            .with_input(serde_json::json!({"action": "list-workflows"}));
        let config = StepConfig::new("s1", "agent");
        let result = plugin.execute(task, config).unwrap();
        assert_eq!(result.status, "success");
    }

    #[test]
    fn execute_list_plugins_returns_success() {
        let plugin = CodingPackPlugin;
        let task = TaskInput::new("t1", "list plugins")
            .with_input(serde_json::json!({"action": "list-plugins"}));
        let config = StepConfig::new("s1", "agent");
        let result = plugin.execute(task, config).unwrap();
        assert_eq!(result.status, "success");
    }

    #[test]
    fn execute_unknown_action_returns_error() {
        let plugin = CodingPackPlugin;
        let task = TaskInput::new("t1", "test")
            .with_input(serde_json::json!({"action": "unknown-action"}));
        let config = StepConfig::new("s1", "agent");
        let err = plugin.execute(task, config).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn execute_missing_input_returns_error() {
        let plugin = CodingPackPlugin;
        let task = TaskInput::new("t1", "test");
        let config = StepConfig::new("s1", "agent");
        let err = plugin.execute(task, config).unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn dashboard_pages_json_is_valid() {
        let plugin = CodingPackPlugin;
        let json = plugin.get_pages_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let pages = parsed.as_array().unwrap();
        assert_eq!(pages.len(), 7);

        // Verify all SDK layout types are present
        let layout_types: Vec<&str> = pages
            .iter()
            .map(|p| p["layout"]["type"].as_str().unwrap())
            .collect();
        assert!(layout_types.contains(&"table"), "missing table layout");
        assert!(layout_types.contains(&"detail"), "missing detail layout");
        assert!(layout_types.contains(&"form"), "missing form layout");
        assert!(layout_types.contains(&"stream"), "missing stream layout");

        // Overview page
        assert_eq!(pages[0]["id"], "overview");
        assert_eq!(pages[0]["path"], "/overview");
        assert_eq!(pages[0]["layout"]["type"], "detail");

        // Workflows table
        assert_eq!(pages[1]["id"], "workflows");
        assert_eq!(pages[1]["layout"]["type"], "table");
        let columns = pages[1]["layout"]["columns"].as_array().unwrap();
        assert!(columns.len() >= 4);

        // Workflow detail
        assert_eq!(pages[2]["id"], "workflow-detail");
        assert_eq!(pages[2]["layout"]["type"], "detail");
        assert_eq!(pages[2]["nav_order"], 99); // hidden from nav (high order)

        // Agents table
        assert_eq!(pages[3]["id"], "agents");
        assert_eq!(pages[3]["layout"]["type"], "table");

        // Status detail
        assert_eq!(pages[4]["id"], "status");
        assert_eq!(pages[4]["layout"]["type"], "detail");

        // Execute form
        assert_eq!(pages[5]["id"], "execute");
        assert_eq!(pages[5]["layout"]["type"], "form");
        let fields = pages[5]["layout"]["fields"].as_array().unwrap();
        assert!(fields.len() >= 2);

        // Logs stream
        assert_eq!(pages[6]["id"], "logs");
        assert_eq!(pages[6]["layout"]["type"], "stream");
    }

    #[test]
    fn dashboard_api_routes_json_is_valid() {
        let plugin = CodingPackPlugin;
        let json = plugin.get_api_routes_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let routes = parsed.as_array().unwrap();
        assert_eq!(routes.len(), 1);
        assert!(routes[0]["prefix"]
            .as_str()
            .unwrap()
            .contains("plugin-coding-pack"));
        // Verify endpoints are documented
        let endpoints = routes[0]["endpoints"].as_array().unwrap();
        assert!(endpoints.len() >= 5);
    }

    #[test]
    fn dashboard_display_customizations_json_is_valid() {
        let plugin = CodingPackPlugin;
        let json = plugin.get_display_customizations_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let customs = parsed.as_array().unwrap();
        assert_eq!(customs.len(), 3);

        // Pack health badge on workflow view
        assert_eq!(customs[0]["id"], "coding-pack-health");
        assert_eq!(customs[0]["target_view"], "workflow");
        assert_eq!(customs[0]["customization"]["type"], "badge");

        // Workflow context fields on task view
        assert_eq!(customs[1]["id"], "coding-workflow-info");
        assert_eq!(customs[1]["target_view"], "task");
        assert_eq!(customs[1]["customization"]["type"], "fields");

        // Agent badge on task view
        assert_eq!(customs[2]["id"], "coding-pack-agent");
        assert_eq!(customs[2]["target_view"], "task");
        assert_eq!(customs[2]["customization"]["type"], "badge");
    }
}
