#![forbid(unsafe_code)]

use plugin_coding_pack::CodingPackPlugin;
use pulse_plugin_sdk::wit_traits::{
    DashboardExtensionPlugin, PluginLifecycle, StepExecutorPlugin,
};
use pulse_plugin_sdk::wit_types::{StepConfig, TaskInput};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let plugin = CodingPackPlugin;
    // Combined adapter: handles step-executor + dashboard-extension methods
    pulse_plugin_sdk::dev_adapter::run_custom_stdio(move |method, params| {
        dispatch_combined(&plugin, method, params)
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_combined(
    plugin: &CodingPackPlugin,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, pulse_plugin_sdk::dev_adapter::DispatchError> {
    use pulse_plugin_sdk::dev_adapter::DispatchError;

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
            serde_json::from_str(&json_str).map_err(|e| {
                DispatchError::Internal(format!("invalid customizations JSON: {e}"))
            })
        }

        _ => Err(DispatchError::MethodNotFound(format!(
            "Method not found: {}",
            method
        ))),
    }
}
