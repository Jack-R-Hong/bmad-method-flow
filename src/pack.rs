use pulse_plugin_sdk::error::WitPluginError;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct CodingPackInput {
    pub action: String,
    /// Optional: target plugin name for plugin-specific actions
    #[serde(default)]
    pub target: Option<String>,
}

/// Execute a pack-level action.
pub fn execute_action(input: &CodingPackInput) -> Result<String, WitPluginError> {
    match input.action.as_str() {
        "validate-pack" => validate_pack(),
        "list-workflows" => list_workflows(),
        "list-plugins" => list_plugins(),
        "status" => pack_status(),
        other => Err(WitPluginError::not_found(format!(
            "Unknown action: '{}'. Available: validate-pack, list-workflows, list-plugins, status",
            other
        ))),
    }
}

fn validate_pack() -> Result<String, WitPluginError> {
    let mut issues = Vec::new();
    let mut ok_count = 0;

    // Check required plugins
    let required_plugins = ["bmad-method", "provider-claude-code"];
    let optional_plugins = ["plugin-git-worktree"];

    for plugin in &required_plugins {
        let path = format!("config/plugins/{}", plugin);
        if Path::new(&path).exists() {
            ok_count += 1;
        } else {
            issues.push(format!("MISSING required plugin: {}", plugin));
        }
    }

    for plugin in &optional_plugins {
        let path = format!("config/plugins/{}", plugin);
        if Path::new(&path).exists() {
            ok_count += 1;
        } else {
            issues.push(format!("MISSING optional plugin: {} (non-blocking)", plugin));
        }
    }

    // Check workflow files
    let workflow_dir = Path::new("config/workflows");
    let workflow_count = if workflow_dir.exists() {
        std::fs::read_dir(workflow_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .and_then(|ext| ext.to_str())
                            == Some("yaml")
                    })
                    .count()
            })
            .unwrap_or(0)
    } else {
        issues.push("MISSING config/workflows directory".to_string());
        0
    };

    let result = serde_json::json!({
        "valid": issues.iter().all(|i| i.contains("optional") || i.contains("non-blocking")),
        "plugins_ok": ok_count,
        "workflows_found": workflow_count,
        "issues": issues,
    });

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}

fn list_workflows() -> Result<String, WitPluginError> {
    let workflow_dir = Path::new("config/workflows");
    let mut workflows = Vec::new();

    if workflow_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(workflow_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        workflows.push(name.to_string());
                    }
                }
            }
        }
    }

    workflows.sort();
    let result = serde_json::json!({
        "workflows": workflows,
        "count": workflows.len(),
    });

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}

fn list_plugins() -> Result<String, WitPluginError> {
    let plugins_dir = Path::new("config/plugins");
    let mut plugins = Vec::new();

    if plugins_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if !name.starts_with('.') {
                        let size = std::fs::metadata(&path)
                            .map(|m| m.len())
                            .unwrap_or(0);
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

    let result = serde_json::json!({
        "plugins": plugins,
        "count": plugins.len(),
    });

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}

fn pack_status() -> Result<String, WitPluginError> {
    let validate = validate_pack()?;
    let workflows = list_workflows()?;
    let plugins = list_plugins()?;

    let result = serde_json::json!({
        "validation": serde_json::from_str::<serde_json::Value>(&validate).unwrap_or_default(),
        "workflows": serde_json::from_str::<serde_json::Value>(&workflows).unwrap_or_default(),
        "plugins": serde_json::from_str::<serde_json::Value>(&plugins).unwrap_or_default(),
    });

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_pack_returns_valid_json() {
        let input = CodingPackInput {
            action: "validate-pack".to_string(),
            target: None,
        };
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("plugins_ok").is_some());
        assert!(parsed.get("workflows_found").is_some());
    }

    #[test]
    fn list_workflows_returns_valid_json() {
        let input = CodingPackInput {
            action: "list-workflows".to_string(),
            target: None,
        };
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("workflows").is_some());
        assert!(parsed.get("count").is_some());
    }

    #[test]
    fn list_plugins_returns_valid_json() {
        let input = CodingPackInput {
            action: "list-plugins".to_string(),
            target: None,
        };
        let result = execute_action(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("plugins").is_some());
    }

    #[test]
    fn unknown_action_returns_not_found() {
        let input = CodingPackInput {
            action: "does-not-exist".to_string(),
            target: None,
        };
        let err = execute_action(&input).unwrap_err();
        assert_eq!(err.code, "not_found");
    }
}
