#![forbid(unsafe_code)]

use plugin_coding_pack::agent_registry::BmadAgentRegistry;
use plugin_coding_pack::config_injector::BmadAgentInjector;
use plugin_coding_pack::tool_provider::BmadToolProvider;
use plugin_coding_pack::CodingPackPlugin;
use pulse_plugin_sdk::wit_traits::{DashboardExtensionPlugin, PluginLifecycle, StepExecutorPlugin};
use pulse_plugin_sdk::wit_types::{StepConfig, TaskInput};
use pulse_plugin_sdk::ConfigInjector;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let plugin = CodingPackPlugin;
    let manifest_path = std::path::PathBuf::from("_bmad/_config/agent-manifest.csv");
    let injector = BmadAgentInjector::new(&manifest_path);
    let tool_provider = BmadToolProvider::new(
        plugin_coding_pack::workspace::WorkspaceConfig::resolve(None),
    );
    let agent_registry = BmadAgentRegistry::new(&manifest_path);
    // Combined adapter: handles step-executor + dashboard-extension + config-injector + tool-provider + agent-definition methods
    pulse_plugin_sdk::dev_adapter::run_custom_stdio(move |method, params| {
        dispatch_combined(&plugin, &injector, &tool_provider, &agent_registry, method, params)
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_combined(
    plugin: &CodingPackPlugin,
    injector: &BmadAgentInjector,
    tool_provider: &BmadToolProvider,
    agent_registry: &BmadAgentRegistry,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, pulse_plugin_sdk::dev_adapter::DispatchError> {
    use pulse_plugin_sdk::dev_adapter::DispatchError;
    use pulse_plugin_sdk::traits::agent_definition::AgentDefinitionProvider;
    use pulse_plugin_sdk::traits::tool_provider::ToolProvider;
    use pulse_plugin_sdk::types::injection::InjectionQuery;
    use pulse_plugin_sdk::types::llm::ToolCall;

    match method {
        // ── Plugin lifecycle ──
        "plugin-lifecycle.get-info" => {
            let info = plugin.get_info();
            serde_json::to_value(&info).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "plugin-lifecycle.health-check" => Ok(serde_json::Value::Bool(plugin.health_check())),

        // ── Step executor ──
        "step-executor.execute" => {
            let task: TaskInput = serde_json::from_value(
                params
                    .get("task")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| DispatchError::InvalidParams(format!("invalid task: {e}")))?;
            let config: StepConfig = serde_json::from_value(
                params
                    .get("config")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| DispatchError::InvalidParams(format!("invalid config: {e}")))?;
            match plugin.execute(task, config) {
                Ok(result) => serde_json::to_value(&result)
                    .map_err(|e| DispatchError::Internal(e.to_string())),
                Err(e) => Err(DispatchError::Internal(e.to_string())),
            }
        }

        // ── Dashboard extension ──
        "dashboard-extension.get-pages-json" => {
            let json_str = plugin.get_pages_json();
            serde_json::from_str(&json_str)
                .map_err(|e| DispatchError::Internal(format!("invalid pages JSON: {e}")))
        }
        "dashboard-extension.get-api-routes-json" => {
            let json_str = plugin.get_api_routes_json();
            serde_json::from_str(&json_str)
                .map_err(|e| DispatchError::Internal(format!("invalid routes JSON: {e}")))
        }
        "dashboard-extension.get-display-customizations-json" => {
            let json_str = plugin.get_display_customizations_json();
            serde_json::from_str(&json_str)
                .map_err(|e| DispatchError::Internal(format!("invalid customizations JSON: {e}")))
        }

        // ── Config injector ──
        "config-injector.injector-name" => {
            let name = injector.injector_name();
            serde_json::to_value(name).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "config-injector.priority" => {
            let priority = injector.priority();
            serde_json::to_value(priority).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "config-injector.applies-to" => {
            let query: InjectionQuery = serde_json::from_value(params)
                .map_err(|e| DispatchError::InvalidParams(format!("invalid query: {e}")))?;
            let applies = injector.applies_to(&query);
            serde_json::to_value(applies).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "config-injector.provide-injections" => {
            let query: InjectionQuery = serde_json::from_value(params)
                .map_err(|e| DispatchError::InvalidParams(format!("invalid query: {e}")))?;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| DispatchError::Internal(format!("runtime error: {e}")))?;
            let result = rt.block_on(injector.provide_injections(&query));
            match result {
                Ok(injections) => serde_json::to_value(injections)
                    .map_err(|e| DispatchError::Internal(e.to_string())),
                Err(e) => Err(DispatchError::Internal(e.to_string())),
            }
        }

        // ── Tool provider ──
        "tool-provider.provider-name" => {
            let name = tool_provider.provider_name();
            serde_json::to_value(name).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "tool-provider.available-tools" => {
            let tools = tool_provider.available_tools();
            serde_json::to_value(tools).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "tool-provider.execute-tool" => {
            let call: ToolCall = serde_json::from_value(params)
                .map_err(|e| DispatchError::InvalidParams(format!("invalid tool call: {e}")))?;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| DispatchError::Internal(format!("runtime error: {e}")))?;
            let result = rt.block_on(tool_provider.execute_tool(call));
            match result {
                Ok(tool_result) => serde_json::to_value(tool_result)
                    .map_err(|e| DispatchError::Internal(e.to_string())),
                Err(e) => Err(DispatchError::Internal(e.to_string())),
            }
        }

        // ── Agent definition provider ──
        "agent-definition.provider-name" => {
            let name = agent_registry.provider_name();
            serde_json::to_value(name).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "agent-definition.list-agents" => {
            let workspace = params
                .get("workspace")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let agents = agent_registry.list_agents(workspace.as_deref());
            serde_json::to_value(agents).map_err(|e| DispatchError::Internal(e.to_string()))
        }
        "agent-definition.get-agent" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    DispatchError::InvalidParams("'name' parameter required".to_string())
                })?;
            let workspace = params
                .get("workspace")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let agent = agent_registry.get_agent(name, workspace.as_deref());
            serde_json::to_value(agent).map_err(|e| DispatchError::Internal(e.to_string()))
        }

        _ => Err(DispatchError::MethodNotFound(format!(
            "Method not found: {}",
            method
        ))),
    }
}
