pub mod pack;
pub mod util;
pub mod validator;

use pulse_plugin_sdk::error::WitPluginError;
use pulse_plugin_sdk::wit_traits::{PluginLifecycle, StepExecutorPlugin};
use pulse_plugin_sdk::wit_types::{PluginDependency, PluginInfo, StepConfig, StepResult, TaskInput};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_health_check_returns_true() {
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
}
