//! BMAD Tool Provider — exposes pack operations as LLM-callable tools.
//!
//! `BmadToolProvider` is a thin adapter over `pack::execute_action()`.
//! It adds zero business logic — only maps tool names to pack actions
//! and wraps results in SDK types.

use async_trait::async_trait;
use pulse_plugin_sdk::traits::tool_provider::ToolProvider;
use pulse_plugin_sdk::types::llm::{ToolCall, ToolDef, ToolSensitivity};
use pulse_plugin_sdk::types::tool_provider::{ToolError, ToolResult};

use crate::pack::{self, CodingPackInput};
use crate::workspace::WorkspaceConfig;

/// Known BMAD tool names.
const TOOL_VALIDATE_PACK: &str = "bmad_validate_pack";
const TOOL_LIST_WORKFLOWS: &str = "bmad_list_workflows";
const TOOL_LIST_PLUGINS: &str = "bmad_list_plugins";
const TOOL_DATA_QUERY: &str = "bmad_data_query";
const TOOL_DATA_MUTATE: &str = "bmad_data_mutate";
// Auto-dev tools
const TOOL_AUTO_DEV_NEXT: &str = "bmad_auto_dev_next";
// Task board control tools
const TOOL_BOARD_LIST: &str = "bmad_board_list";
const TOOL_BOARD_CREATE_TASK: &str = "bmad_board_create_task";
const TOOL_BOARD_UPDATE_TASK: &str = "bmad_board_update_task";
const TOOL_BOARD_ADD_COMMENT: &str = "bmad_board_add_comment";
const TOOL_BOARD_ADD_SUBTASK: &str = "bmad_board_add_subtask";
const TOOL_BOARD_TOGGLE_SUBTASK: &str = "bmad_board_toggle_subtask";

/// Tool provider that exposes BMAD coding pack operations as LLM-callable tools.
///
/// Holds a `WorkspaceConfig` so tool calls resolve paths relative to the same
/// workspace the provider was constructed with.
pub struct BmadToolProvider {
    config: WorkspaceConfig,
}

impl BmadToolProvider {
    /// Create a new `BmadToolProvider` with the given workspace configuration.
    pub fn new(config: WorkspaceConfig) -> Self {
        Self { config }
    }

    /// Handle board control tools. Returns `Ok(None)` if the tool is not a board tool.
    fn execute_board_tool(
        &self,
        call: &ToolCall,
    ) -> Result<Option<ToolResult>, ToolError> {
        use crate::board_store;

        let base_dir = &self.config.base_dir;

        let result = match call.name.as_str() {
            TOOL_BOARD_LIST => {
                let data = if board_store::store_exists(base_dir) {
                    board_store::get_assignments_list_from_store(&self.config)
                } else {
                    Ok(serde_json::json!({ "items": [] }))
                };
                let mut val = data.map_err(|e| ToolError::execution_error(e.to_string()))?;
                // Filter by status if provided
                if let Some(status) = call.arguments.get("status").and_then(|v| v.as_str()) {
                    if let Some(items) = val.get_mut("items").and_then(|v| v.as_array_mut()) {
                        items.retain(|item| item["status"].as_str() == Some(status));
                    }
                }
                val
            }
            TOOL_BOARD_CREATE_TASK => {
                board_store::create_assignment(base_dir, &call.arguments)
                    .map_err(|e| ToolError::execution_error(e.to_string()))?
            }
            TOOL_BOARD_UPDATE_TASK => {
                let task_id = call
                    .arguments
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_arguments("'task_id' parameter required".to_string())
                    })?;
                board_store::update_assignment(base_dir, task_id, &call.arguments)
                    .map_err(|e| ToolError::execution_error(e.to_string()))?
            }
            TOOL_BOARD_ADD_COMMENT => {
                let task_id = call
                    .arguments
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_arguments("'task_id' parameter required".to_string())
                    })?;
                board_store::add_comment(base_dir, task_id, &call.arguments)
                    .map_err(|e| ToolError::execution_error(e.to_string()))?
            }
            TOOL_BOARD_ADD_SUBTASK => {
                let task_id = call
                    .arguments
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_arguments("'task_id' parameter required".to_string())
                    })?;
                board_store::add_subtask(base_dir, task_id, &call.arguments)
                    .map_err(|e| ToolError::execution_error(e.to_string()))?
            }
            TOOL_BOARD_TOGGLE_SUBTASK => {
                let task_id = call
                    .arguments
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_arguments("'task_id' parameter required".to_string())
                    })?;
                let subtask_id = call
                    .arguments
                    .get("subtask_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::invalid_arguments("'subtask_id' parameter required".to_string())
                    })?;
                board_store::toggle_subtask(base_dir, task_id, subtask_id)
                    .map_err(|e| ToolError::execution_error(e.to_string()))?
            }
            _ => return Ok(None),
        };

        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| result.to_string());
        Ok(Some(ToolResult::success(content)))
    }
}

// Compile-time assertion that BmadToolProvider is Send + Sync.
const _: () = {
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        _assert_send_sync::<BmadToolProvider>();
    }
};

/// Map a tool name to its corresponding pack action.
///
/// Convention: strip `bmad_` prefix, replace `_` with `-`.
/// Returns `None` for unrecognized tool names.
fn tool_name_to_action(name: &str) -> Option<&'static str> {
    match name {
        TOOL_VALIDATE_PACK => Some("validate-pack"),
        TOOL_LIST_WORKFLOWS => Some("list-workflows"),
        TOOL_LIST_PLUGINS => Some("list-plugins"),
        TOOL_DATA_QUERY => Some("data-query"),
        TOOL_DATA_MUTATE => Some("data-mutate"),
        _ => None,
    }
}

#[async_trait]
impl ToolProvider for BmadToolProvider {
    fn provider_name(&self) -> &str {
        "bmad-coding-pack"
    }

    // applies_to uses the default implementation (returns true for all provider/model combos)

    fn available_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: TOOL_VALIDATE_PACK.to_string(),
                description: "Validate the coding pack health — checks plugins, workflows, config"
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                sensitivity: ToolSensitivity::Low,
            },
            ToolDef {
                name: TOOL_LIST_WORKFLOWS.to_string(),
                description:
                    "List all available BMAD workflows with their categories and step counts"
                        .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                sensitivity: ToolSensitivity::Low,
            },
            ToolDef {
                name: TOOL_LIST_PLUGINS.to_string(),
                description: "List installed plugins and their health status".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                sensitivity: ToolSensitivity::Low,
            },
            ToolDef {
                name: TOOL_DATA_QUERY.to_string(),
                description: "Query dashboard data endpoints".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "endpoint": {
                            "type": "string",
                            "description": "Data endpoint path"
                        }
                    },
                    "required": ["endpoint"]
                }),
                sensitivity: ToolSensitivity::Low,
            },
            ToolDef {
                name: TOOL_DATA_MUTATE.to_string(),
                description: "Mutate board data (update status, create/update epics and stories)"
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "endpoint": {
                            "type": "string",
                            "description": "Mutation endpoint path (e.g. board/sync, board/status/{id}, board/epics, board/stories)"
                        },
                        "payload": {
                            "type": "object",
                            "description": "Mutation payload"
                        }
                    },
                    "required": ["endpoint"]
                }),
                sensitivity: ToolSensitivity::Medium,
            },
            // ── Task Board control tools ──────────────────────────────
            ToolDef {
                name: TOOL_BOARD_LIST.to_string(),
                description: "List all tasks on the board with their status, assignee, and progress. Optionally filter by status."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "status": {
                            "type": "string",
                            "description": "Filter by status: backlog, ready-for-dev, in-progress, review, done",
                            "enum": ["backlog", "ready-for-dev", "in-progress", "review", "done"]
                        }
                    },
                    "required": []
                }),
                sensitivity: ToolSensitivity::Low,
            },
            ToolDef {
                name: TOOL_BOARD_CREATE_TASK.to_string(),
                description: "Create a new task on the board. Returns the created task with its auto-generated ID."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Task title"
                        },
                        "description": {
                            "type": "string",
                            "description": "Task description"
                        },
                        "status": {
                            "type": "string",
                            "description": "Initial status (default: backlog)",
                            "enum": ["backlog", "ready-for-dev", "in-progress", "review", "done"]
                        },
                        "assignee": {
                            "type": "string",
                            "description": "Who is assigned to this task"
                        },
                        "priority": {
                            "type": "string",
                            "description": "Priority level (default: medium)",
                            "enum": ["low", "medium", "high", "critical"]
                        },
                        "labels": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Labels/tags for the task"
                        },
                        "tasks": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Sub-task titles to create as checklist items"
                        }
                    },
                    "required": ["title"]
                }),
                sensitivity: ToolSensitivity::Medium,
            },
            ToolDef {
                name: TOOL_BOARD_UPDATE_TASK.to_string(),
                description: "Update an existing task on the board (change status, title, assignee, priority, etc.)"
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "The task ID to update"
                        },
                        "title": {
                            "type": "string",
                            "description": "New title"
                        },
                        "status": {
                            "type": "string",
                            "description": "New status",
                            "enum": ["backlog", "ready-for-dev", "in-progress", "review", "done"]
                        },
                        "description": {
                            "type": "string",
                            "description": "New description"
                        },
                        "assignee": {
                            "type": "string",
                            "description": "New assignee"
                        },
                        "priority": {
                            "type": "string",
                            "description": "New priority",
                            "enum": ["low", "medium", "high", "critical"]
                        },
                        "labels": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "New labels"
                        }
                    },
                    "required": ["task_id"]
                }),
                sensitivity: ToolSensitivity::Medium,
            },
            ToolDef {
                name: TOOL_BOARD_ADD_COMMENT.to_string(),
                description: "Add a comment to a task on the board. Use this to record observations, progress notes, or decisions."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "The task ID to comment on"
                        },
                        "content": {
                            "type": "string",
                            "description": "Comment text"
                        },
                        "author": {
                            "type": "string",
                            "description": "Comment author (default: LLM Agent)"
                        }
                    },
                    "required": ["task_id", "content"]
                }),
                sensitivity: ToolSensitivity::Medium,
            },
            ToolDef {
                name: TOOL_BOARD_ADD_SUBTASK.to_string(),
                description: "Add a sub-task (checklist item) to a task on the board."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "The task ID to add a sub-task to"
                        },
                        "title": {
                            "type": "string",
                            "description": "Sub-task title"
                        }
                    },
                    "required": ["task_id", "title"]
                }),
                sensitivity: ToolSensitivity::Medium,
            },
            ToolDef {
                name: TOOL_BOARD_TOGGLE_SUBTASK.to_string(),
                description: "Toggle a sub-task's completion status (done/not done)."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "The task ID containing the sub-task"
                        },
                        "subtask_id": {
                            "type": "string",
                            "description": "The sub-task ID to toggle"
                        }
                    },
                    "required": ["task_id", "subtask_id"]
                }),
                sensitivity: ToolSensitivity::Medium,
            },
            // ── Auto-dev tool ────────────────────────────────────────
            ToolDef {
                name: TOOL_AUTO_DEV_NEXT.to_string(),
                description: "Pick the next ready-for-dev task from the board, run its workflow, validate with tests, and update the board. Returns the result or idle status if no tasks are ready."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                sensitivity: ToolSensitivity::High,
            },
        ]
    }

    async fn execute_tool(&self, call: ToolCall) -> Result<ToolResult, ToolError> {
        // Auto-dev tool
        if call.name == TOOL_AUTO_DEV_NEXT {
            let result = crate::auto_dev::auto_dev_next(&self.config)
                .map_err(|e| ToolError::execution_error(e.to_string()))?;
            let json = match result {
                Some(r) => serde_json::to_string_pretty(&r).unwrap_or_default(),
                None => r#"{"status":"idle","message":"No ready-for-dev tasks found"}"#.to_string(),
            };
            return Ok(ToolResult::success(json));
        }

        // Board control tools route directly to board_store
        if let Some(result) = self.execute_board_tool(&call)? {
            return Ok(result);
        }

        let action =
            tool_name_to_action(&call.name).ok_or_else(|| ToolError::not_found(&call.name))?;

        let needs_endpoint = call.name == TOOL_DATA_QUERY || call.name == TOOL_DATA_MUTATE;
        let endpoint = if needs_endpoint {
            let ep = call
                .arguments
                .get("endpoint")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::invalid_arguments(format!("{} requires 'endpoint' parameter", call.name))
                })?;
            Some(ep.to_string())
        } else {
            None
        };

        let payload = if call.name == TOOL_DATA_MUTATE {
            call.arguments.get("payload").cloned()
        } else {
            None
        };

        let input = CodingPackInput {
            action: action.to_string(),
            target: None,
            workflow_id: None,
            input: None,
            endpoint,
            payload,
            workspace_dir: Some(self.config.base_dir.to_string_lossy().to_string()),
            workspace: None,
        };

        pack::execute_action(&input)
            .map(ToolResult::success)
            .map_err(|e| ToolError::execution_error(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_workspace_config() -> WorkspaceConfig {
        WorkspaceConfig::resolve(None)
    }

    #[test]
    fn test_provider_name() {
        let provider = BmadToolProvider::new(test_workspace_config());
        assert_eq!(provider.provider_name(), "bmad-coding-pack");
    }

    #[test]
    fn test_available_tools_count_and_names() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let tools = provider.available_tools();
        assert_eq!(tools.len(), 12);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        // Core tools
        assert!(names.contains(&"bmad_validate_pack"));
        assert!(names.contains(&"bmad_list_workflows"));
        assert!(names.contains(&"bmad_list_plugins"));
        assert!(names.contains(&"bmad_data_query"));
        assert!(names.contains(&"bmad_data_mutate"));
        // Board control tools
        assert!(names.contains(&"bmad_board_list"));
        assert!(names.contains(&"bmad_board_create_task"));
        assert!(names.contains(&"bmad_board_update_task"));
        assert!(names.contains(&"bmad_board_add_comment"));
        assert!(names.contains(&"bmad_board_add_subtask"));
        assert!(names.contains(&"bmad_board_toggle_subtask"));
        // Auto-dev tool
        assert!(names.contains(&"bmad_auto_dev_next"));

        // Verify descriptions are non-empty
        for tool in &tools {
            assert!(
                !tool.description.is_empty(),
                "tool {} has empty description",
                tool.name
            );
        }

        // Low sensitivity: read-only tools
        let low_tools = ["bmad_validate_pack", "bmad_list_workflows", "bmad_list_plugins", "bmad_data_query", "bmad_board_list"];
        let high_tools = ["bmad_auto_dev_next"];
        for tool in &tools {
            if low_tools.contains(&tool.name.as_str()) {
                assert_eq!(
                    tool.sensitivity,
                    ToolSensitivity::Low,
                    "tool {} should have Low sensitivity",
                    tool.name
                );
            } else if high_tools.contains(&tool.name.as_str()) {
                assert_eq!(
                    tool.sensitivity,
                    ToolSensitivity::High,
                    "tool {} should have High sensitivity",
                    tool.name
                );
            } else {
                assert_eq!(
                    tool.sensitivity,
                    ToolSensitivity::Medium,
                    "tool {} should have Medium sensitivity",
                    tool.name
                );
            }
        }
    }

    #[tokio::test]
    async fn test_execute_validate_pack() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let call = ToolCall {
            id: "test-1".into(),
            name: "bmad_validate_pack".into(),
            arguments: serde_json::json!({}),
        };
        let result = provider.execute_tool(call).await.unwrap();
        assert!(!result.is_error);
        // Result should be valid JSON containing pack validation data
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert!(parsed.get("plugins_ok").is_some() || parsed.get("valid").is_some());
    }

    #[tokio::test]
    async fn test_execute_unknown_tool() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let call = ToolCall {
            id: "test-2".into(),
            name: "nonexistent_tool".into(),
            arguments: serde_json::json!({}),
        };
        let result = provider.execute_tool(call).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_execute_list_workflows() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let call = ToolCall {
            id: "test-3".into(),
            name: "bmad_list_workflows".into(),
            arguments: serde_json::json!({}),
        };
        let result = provider.execute_tool(call).await.unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert!(parsed.get("workflows").is_some());
    }

    #[tokio::test]
    async fn test_execute_list_plugins() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let call = ToolCall {
            id: "test-4".into(),
            name: "bmad_list_plugins".into(),
            arguments: serde_json::json!({}),
        };
        let result = provider.execute_tool(call).await.unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert!(parsed.get("plugins").is_some());
    }

    #[tokio::test]
    async fn test_execute_data_query_with_endpoint() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let call = ToolCall {
            id: "test-5".into(),
            name: "bmad_data_query".into(),
            arguments: serde_json::json!({"endpoint": "status"}),
        };
        let result = provider.execute_tool(call).await.unwrap();
        assert!(!result.is_error);
        // Result should be valid JSON
        let _parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    }

    #[tokio::test]
    async fn test_execute_data_query_missing_endpoint() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let call = ToolCall {
            id: "test-6".into(),
            name: "bmad_data_query".into(),
            arguments: serde_json::json!({}),
        };
        let result = provider.execute_tool(call).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::InvalidArguments(_)
        ));
    }

    // ── Board tool dispatch tests ────────────────────────────────────

    fn temp_provider() -> (tempfile::TempDir, BmadToolProvider) {
        let dir = tempfile::tempdir().unwrap();
        let config = WorkspaceConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        (dir, BmadToolProvider::new(config))
    }

    #[tokio::test]
    async fn test_board_create_task_via_tool() {
        let (_dir, provider) = temp_provider();
        let call = ToolCall {
            id: "bt-1".into(),
            name: "bmad_board_create_task".into(),
            arguments: serde_json::json!({
                "title": "Test Task",
                "status": "backlog",
                "assignee": "dev-1"
            }),
        };
        let result = provider.execute_tool(call).await.unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed["title"], "Test Task");
        assert_eq!(parsed["status"], "backlog");
    }

    #[tokio::test]
    async fn test_board_list_empty() {
        let (_dir, provider) = temp_provider();
        let call = ToolCall {
            id: "bt-2".into(),
            name: "bmad_board_list".into(),
            arguments: serde_json::json!({}),
        };
        let result = provider.execute_tool(call).await.unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_board_list_with_status_filter() {
        let (_dir, provider) = temp_provider();
        // Create two tasks with different statuses
        provider
            .execute_tool(ToolCall {
                id: "s1".into(),
                name: "bmad_board_create_task".into(),
                arguments: serde_json::json!({"title": "A", "status": "backlog"}),
            })
            .await
            .unwrap();
        provider
            .execute_tool(ToolCall {
                id: "s2".into(),
                name: "bmad_board_create_task".into(),
                arguments: serde_json::json!({"title": "B", "status": "in-progress"}),
            })
            .await
            .unwrap();

        // List with filter
        let result = provider
            .execute_tool(ToolCall {
                id: "bt-3".into(),
                name: "bmad_board_list".into(),
                arguments: serde_json::json!({"status": "backlog"}),
            })
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let items = parsed["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "A");
    }

    #[tokio::test]
    async fn test_board_update_task_via_tool() {
        let (_dir, provider) = temp_provider();
        // Create a task first
        let create_result = provider
            .execute_tool(ToolCall {
                id: "s1".into(),
                name: "bmad_board_create_task".into(),
                arguments: serde_json::json!({"title": "Original"}),
            })
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_str(&create_result.content).unwrap();
        let task_id = created["id"].as_str().unwrap();

        // Update it
        let result = provider
            .execute_tool(ToolCall {
                id: "bt-4".into(),
                name: "bmad_board_update_task".into(),
                arguments: serde_json::json!({
                    "task_id": task_id,
                    "title": "Updated",
                    "status": "in-progress"
                }),
            })
            .await
            .unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed["title"], "Updated");
        assert_eq!(parsed["status"], "in-progress");
    }

    #[tokio::test]
    async fn test_board_update_task_missing_id() {
        let (_dir, provider) = temp_provider();
        let result = provider
            .execute_tool(ToolCall {
                id: "bt-5".into(),
                name: "bmad_board_update_task".into(),
                arguments: serde_json::json!({"title": "X"}),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_board_add_comment_via_tool() {
        let (_dir, provider) = temp_provider();
        // Create a task
        let create_result = provider
            .execute_tool(ToolCall {
                id: "s1".into(),
                name: "bmad_board_create_task".into(),
                arguments: serde_json::json!({"title": "Task"}),
            })
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_str(&create_result.content).unwrap();
        let task_id = created["id"].as_str().unwrap();

        // Add comment
        let result = provider
            .execute_tool(ToolCall {
                id: "bt-6".into(),
                name: "bmad_board_add_comment".into(),
                arguments: serde_json::json!({
                    "task_id": task_id,
                    "content": "Progress update"
                }),
            })
            .await
            .unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed["content"], "Progress update");
        assert_eq!(parsed["author"], "LLM Agent");
    }

    #[tokio::test]
    async fn test_board_add_subtask_via_tool() {
        let (_dir, provider) = temp_provider();
        let create_result = provider
            .execute_tool(ToolCall {
                id: "s1".into(),
                name: "bmad_board_create_task".into(),
                arguments: serde_json::json!({"title": "Task"}),
            })
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_str(&create_result.content).unwrap();
        let task_id = created["id"].as_str().unwrap();

        let result = provider
            .execute_tool(ToolCall {
                id: "bt-7".into(),
                name: "bmad_board_add_subtask".into(),
                arguments: serde_json::json!({
                    "task_id": task_id,
                    "title": "Write tests"
                }),
            })
            .await
            .unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed["title"], "Write tests");
        assert!(!parsed["done"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_board_toggle_subtask_via_tool() {
        let (_dir, provider) = temp_provider();
        // Create task with subtask
        let create_result = provider
            .execute_tool(ToolCall {
                id: "s1".into(),
                name: "bmad_board_create_task".into(),
                arguments: serde_json::json!({
                    "title": "Task",
                    "tasks": ["item 1"]
                }),
            })
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_str(&create_result.content).unwrap();
        let task_id = created["id"].as_str().unwrap();

        // Toggle st-1
        let result = provider
            .execute_tool(ToolCall {
                id: "bt-8".into(),
                name: "bmad_board_toggle_subtask".into(),
                arguments: serde_json::json!({
                    "task_id": task_id,
                    "subtask_id": "st-1"
                }),
            })
            .await
            .unwrap();
        assert!(!result.is_error);
        let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert!(parsed["done"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_board_toggle_subtask_missing_params() {
        let (_dir, provider) = temp_provider();
        // Missing subtask_id
        let result = provider
            .execute_tool(ToolCall {
                id: "bt-9".into(),
                name: "bmad_board_toggle_subtask".into(),
                arguments: serde_json::json!({"task_id": "x"}),
            })
            .await;
        assert!(result.is_err());
    }
}
