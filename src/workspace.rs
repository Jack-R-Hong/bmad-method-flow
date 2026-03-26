use serde::Deserialize;
use std::path::PathBuf;

/// Per-workspace configuration, resolved once per request.
/// All directory paths are resolved relative to `base_dir`.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Root directory for this workspace
    pub base_dir: PathBuf,
    /// Resolved path to plugins directory
    pub plugins_dir: PathBuf,
    /// Resolved path to workflows directory
    pub workflows_dir: PathBuf,
    /// Workflow filtering rules
    pub workflows: WorkflowFilter,
    /// Default model/provider settings
    pub defaults: DefaultSettings,
    /// When true, the executor skips the two-stage persona fetch and lets
    /// the injection pipeline (BmadAgentInjector) compose system prompts.
    pub use_injection_pipeline: bool,
}

/// Controls which workflows are available in this workspace.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowFilter {
    /// If non-empty, only these workflow IDs are available
    #[serde(default)]
    pub enabled: Vec<String>,
    /// These workflow IDs are excluded (takes priority over enabled)
    #[serde(default)]
    pub disabled: Vec<String>,
}

/// Default settings that can be overridden per workspace.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DefaultSettings {
    /// Default model tier for agent steps (e.g. "fast", "balanced")
    #[serde(default)]
    pub default_model: Option<String>,
    /// Maximum budget in USD for provider-claude-code
    #[serde(default)]
    pub max_budget_usd: Option<f64>,
    /// Memory provider settings
    #[serde(default)]
    pub memory: Option<MemorySettings>,
}

/// Memory / knowledge graph settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MemorySettings {
    /// Provider name: "gitnexus" | "greptile" | "none"
    #[serde(default)]
    pub provider: Option<String>,
    /// Re-index after each commit
    #[serde(default)]
    pub auto_reindex: Option<bool>,
}

/// Raw YAML structure matching config/config.yaml, extended with workspace fields.
#[derive(Debug, Deserialize)]
struct ConfigYaml {
    /// Plugin binary directory (e.g. "config/plugins")
    #[serde(default)]
    plugin_dir: Option<String>,
    /// Workflow YAML directory (e.g. "config/workflows")
    #[serde(default)]
    workflows_dir: Option<String>,
    /// Workflow filtering
    #[serde(default)]
    workflows: Option<WorkflowFilter>,
    /// Default model/budget settings
    #[serde(default)]
    defaults: Option<DefaultSettings>,
    /// Memory settings (top-level in existing config.yaml)
    #[serde(default)]
    memory: Option<MemorySettings>,
    /// When true, skip persona fetch and let the injection pipeline handle system prompts
    #[serde(default)]
    use_injection_pipeline: Option<bool>,
}

const DEFAULT_PLUGINS_DIR: &str = "config/plugins";
const DEFAULT_WORKFLOWS_DIR: &str = "config/workflows";

impl WorkspaceConfig {
    /// Create config from a base directory, reading config/config.yaml if present.
    pub fn from_base_dir(base_dir: impl Into<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        let config_path = base_dir.join("config/config.yaml");

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(yaml) = serde_yaml::from_str::<ConfigYaml>(&content) {
                let plugins_dir =
                    base_dir.join(yaml.plugin_dir.as_deref().unwrap_or(DEFAULT_PLUGINS_DIR));
                let workflows_dir = base_dir.join(
                    yaml.workflows_dir
                        .as_deref()
                        .unwrap_or(DEFAULT_WORKFLOWS_DIR),
                );

                // Merge memory into defaults if defaults.memory is not set
                let defaults = match yaml.defaults {
                    Some(mut d) => {
                        if d.memory.is_none() {
                            d.memory = yaml.memory;
                        }
                        d
                    }
                    None => DefaultSettings {
                        memory: yaml.memory,
                        ..Default::default()
                    },
                };

                return Self {
                    base_dir,
                    plugins_dir,
                    workflows_dir,
                    workflows: yaml.workflows.unwrap_or_default(),
                    defaults,
                    use_injection_pipeline: yaml.use_injection_pipeline.unwrap_or(false),
                };
            }
        }

        // Fallback: no config file or parse error — use defaults
        Self::default_for(base_dir)
    }

    /// Create default config with hardcoded relative paths from base_dir.
    fn default_for(base_dir: PathBuf) -> Self {
        Self {
            plugins_dir: base_dir.join(DEFAULT_PLUGINS_DIR),
            workflows_dir: base_dir.join(DEFAULT_WORKFLOWS_DIR),
            base_dir,
            workflows: WorkflowFilter::default(),
            defaults: DefaultSettings::default(),
            use_injection_pipeline: false,
        }
    }

    /// Resolve workspace config from an optional workspace_dir override.
    /// Priority: workspace_dir arg > PULSE_WORKSPACE_DIR env > "."
    pub fn resolve(workspace_dir: Option<&str>) -> Self {
        if let Some(dir) = workspace_dir {
            return Self::from_base_dir(dir);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(dir) = std::env::var("PULSE_WORKSPACE_DIR") {
            if !dir.is_empty() {
                return Self::from_base_dir(dir);
            }
        }

        Self::from_base_dir(".")
    }

    /// Check if a workflow ID is allowed by the filter rules.
    /// Disabled list takes priority. If enabled list is non-empty, only those are allowed.
    pub fn is_workflow_enabled(&self, workflow_id: &str) -> bool {
        if self.workflows.disabled.iter().any(|d| d == workflow_id) {
            return false;
        }
        if !self.workflows.enabled.is_empty() {
            return self.workflows.enabled.iter().any(|e| e == workflow_id);
        }
        true
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self::default_for(PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_relative_paths() {
        let config = WorkspaceConfig::default();
        assert_eq!(config.base_dir, PathBuf::from("."));
        // Paths are relative to "." — may be "./config/plugins" or "config/plugins"
        // depending on whether config.yaml exists and specifies plugin_dir
        assert!(
            config.plugins_dir.ends_with("config/plugins"),
            "plugins_dir should end with config/plugins, got: {:?}",
            config.plugins_dir
        );
        assert!(
            config.workflows_dir.ends_with("config/workflows"),
            "workflows_dir should end with config/workflows, got: {:?}",
            config.workflows_dir
        );
    }

    #[test]
    fn from_base_dir_resolves_paths() {
        let config = WorkspaceConfig::from_base_dir("/some/workspace");
        assert_eq!(config.base_dir, PathBuf::from("/some/workspace"));
        assert_eq!(
            config.plugins_dir,
            PathBuf::from("/some/workspace/config/plugins")
        );
        assert_eq!(
            config.workflows_dir,
            PathBuf::from("/some/workspace/config/workflows")
        );
    }

    #[test]
    fn resolve_with_explicit_dir() {
        let config = WorkspaceConfig::resolve(Some("/tmp/test-workspace"));
        assert_eq!(config.base_dir, PathBuf::from("/tmp/test-workspace"));
    }

    #[test]
    fn resolve_without_dir_falls_back_to_cwd() {
        let config = WorkspaceConfig::resolve(None);
        // Without PULSE_WORKSPACE_DIR set, should fall back to "."
        assert_eq!(config.base_dir, PathBuf::from("."));
    }

    #[test]
    fn workflow_filter_all_enabled_by_default() {
        let config = WorkspaceConfig::default();
        assert!(config.is_workflow_enabled("coding-quick-dev"));
        assert!(config.is_workflow_enabled("bootstrap-cycle"));
        assert!(config.is_workflow_enabled("anything"));
    }

    #[test]
    fn workflow_filter_disabled_takes_priority() {
        let config = WorkspaceConfig {
            workflows: WorkflowFilter {
                enabled: vec!["coding-quick-dev".to_string(), "coding-bug-fix".to_string()],
                disabled: vec!["coding-quick-dev".to_string()],
            },
            ..WorkspaceConfig::default()
        };
        assert!(!config.is_workflow_enabled("coding-quick-dev")); // disabled wins
        assert!(config.is_workflow_enabled("coding-bug-fix"));
        assert!(!config.is_workflow_enabled("other")); // not in enabled list
    }

    #[test]
    fn workflow_filter_enabled_list_restricts() {
        let config = WorkspaceConfig {
            workflows: WorkflowFilter {
                enabled: vec!["coding-quick-dev".to_string()],
                disabled: vec![],
            },
            ..WorkspaceConfig::default()
        };
        assert!(config.is_workflow_enabled("coding-quick-dev"));
        assert!(!config.is_workflow_enabled("coding-feature-dev"));
    }

    #[test]
    fn config_yaml_parsing() {
        let yaml = r#"
plugin_dir: "custom/plugins"
workflows_dir: "custom/workflows"
workflows:
  disabled:
    - bootstrap-cycle
defaults:
  default_model: "fast"
  max_budget_usd: 5.0
memory:
  provider: greptile
  auto_reindex: false
"#;
        let parsed: ConfigYaml = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.plugin_dir.as_deref(), Some("custom/plugins"));
        assert_eq!(parsed.workflows_dir.as_deref(), Some("custom/workflows"));
        assert_eq!(parsed.workflows.as_ref().unwrap().disabled.len(), 1);
        assert_eq!(
            parsed.defaults.as_ref().unwrap().default_model.as_deref(),
            Some("fast")
        );
        assert_eq!(
            parsed.memory.as_ref().unwrap().provider.as_deref(),
            Some("greptile")
        );
    }

    #[test]
    fn config_yaml_backward_compatible() {
        // Existing config.yaml format without new fields should parse fine
        let yaml = r#"
db_path: "pulse.db"
log_level: "info"
plugin_dir: "config/plugins"
memory:
  provider: gitnexus
  auto_reindex: true
"#;
        let parsed: ConfigYaml = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.plugin_dir.as_deref(), Some("config/plugins"));
        assert!(parsed.workflows_dir.is_none());
        assert!(parsed.workflows.is_none());
        assert!(parsed.defaults.is_none());
        assert_eq!(
            parsed.memory.as_ref().unwrap().provider.as_deref(),
            Some("gitnexus")
        );
    }

    // ── use_injection_pipeline flag (Story 21-2) ─────────────────────

    #[test]
    fn test_injection_pipeline_flag_default_false() {
        // Config YAML without the flag — should default to false
        let yaml = r#"
plugin_dir: "config/plugins"
memory:
  provider: gitnexus
"#;
        let parsed: ConfigYaml = serde_yaml::from_str(yaml).unwrap();
        assert!(parsed.use_injection_pipeline.is_none());

        // WorkspaceConfig should default to false
        let config = WorkspaceConfig::default();
        assert!(!config.use_injection_pipeline);
    }

    #[test]
    fn test_injection_pipeline_flag_parsed_true() {
        let yaml = r#"
plugin_dir: "config/plugins"
use_injection_pipeline: true
"#;
        let parsed: ConfigYaml = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.use_injection_pipeline, Some(true));
    }
}
