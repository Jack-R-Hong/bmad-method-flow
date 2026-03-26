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
        ]
    }

    async fn execute_tool(&self, call: ToolCall) -> Result<ToolResult, ToolError> {
        let action =
            tool_name_to_action(&call.name).ok_or_else(|| ToolError::not_found(&call.name))?;

        let endpoint = if call.name == TOOL_DATA_QUERY {
            let ep = call
                .arguments
                .get("endpoint")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::invalid_arguments("bmad_data_query requires 'endpoint' parameter")
                })?;
            Some(ep.to_string())
        } else {
            None
        };

        let input = CodingPackInput {
            action: action.to_string(),
            target: None,
            workflow_id: None,
            input: None,
            endpoint,
            workspace_dir: Some(self.config.base_dir.to_string_lossy().to_string()),
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
    fn test_available_tools_returns_four() {
        let provider = BmadToolProvider::new(test_workspace_config());
        let tools = provider.available_tools();
        assert_eq!(tools.len(), 4);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"bmad_validate_pack"));
        assert!(names.contains(&"bmad_list_workflows"));
        assert!(names.contains(&"bmad_list_plugins"));
        assert!(names.contains(&"bmad_data_query"));

        // Verify descriptions are non-empty
        for tool in &tools {
            assert!(
                !tool.description.is_empty(),
                "tool {} has empty description",
                tool.name
            );
        }

        // Verify all tools have Low sensitivity
        for tool in &tools {
            assert_eq!(
                tool.sensitivity,
                ToolSensitivity::Low,
                "tool {} should have Low sensitivity",
                tool.name
            );
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
}
