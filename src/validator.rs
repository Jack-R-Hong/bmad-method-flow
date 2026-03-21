use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Deserialized workflow YAML structure for validation.
#[derive(Debug, Deserialize)]
struct WorkflowYaml {
    name: Option<String>,
    version: Option<serde_yaml::Value>,
    #[serde(default)]
    steps: Option<Vec<StepYaml>>,
    #[serde(default)]
    requires: Option<Vec<RequiresEntry>>,
}

#[derive(Debug, Deserialize)]
struct StepYaml {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type")]
    step_type: Option<String>,
    #[serde(default)]
    executor: Option<String>,
    #[serde(default)]
    config: Option<StepConfigYaml>,
    #[serde(default)]
    depends_on: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StepConfigYaml {
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
    #[serde(default)]
    context_from: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RequiresEntry {
    plugin: String,
}

/// Detect cycles in a directed graph via DFS.
fn has_cycle(adj: &HashMap<String, Vec<String>>) -> bool {
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    fn dfs(
        node: &str,
        adj: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(node.to_string());
        in_stack.insert(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor.as_str()) {
                    if dfs(neighbor, adj, visited, in_stack) {
                        return true;
                    }
                } else if in_stack.contains(neighbor.as_str()) {
                    return true;
                }
            }
        }

        in_stack.remove(node);
        false
    }

    for node in adj.keys() {
        if !visited.contains(node.as_str()) && dfs(node, adj, &mut visited, &mut in_stack) {
            return true;
        }
    }
    false
}

/// Validate that a workflow YAML file has required structure.
pub fn validate_workflow_file(path: &Path) -> Result<ValidationResult, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;

    let mut issues = Vec::new();

    // Parse YAML properly
    let workflow: WorkflowYaml = match serde_yaml::from_str(&content) {
        Ok(w) => w,
        Err(e) => {
            issues.push(format!("invalid YAML: {}", e));
            return Ok(ValidationResult {
                file: path.display().to_string(),
                valid: false,
                issues,
            });
        }
    };

    // Check required top-level fields
    if workflow.name.as_ref().is_none_or(|n| n.is_empty()) {
        issues.push("missing or empty 'name' field".to_string());
    }
    if workflow.version.is_none() {
        issues.push("missing 'version' field".to_string());
    }

    let steps = match &workflow.steps {
        Some(s) if !s.is_empty() => s,
        Some(_) => {
            issues.push("'steps' array is empty".to_string());
            return Ok(ValidationResult {
                file: path.display().to_string(),
                valid: issues.is_empty(),
                issues,
            });
        }
        None => {
            issues.push("missing 'steps' field".to_string());
            return Ok(ValidationResult {
                file: path.display().to_string(),
                valid: issues.is_empty(),
                issues,
            });
        }
    };

    // Validate each step
    for (i, step) in steps.iter().enumerate() {
        let step_label = step
            .id
            .as_deref()
            .map(|id| format!("step '{}'", id))
            .unwrap_or_else(|| format!("step[{}]", i));

        if step.id.is_none() {
            issues.push(format!("{}: missing 'id' field", step_label));
        }

        // Agent steps must have system_prompt
        if step.step_type.as_deref() == Some("agent") {
            let has_prompt = step
                .config
                .as_ref()
                .and_then(|c| c.system_prompt.as_ref())
                .is_some_and(|p| !p.is_empty());
            if !has_prompt {
                issues.push(format!(
                    "{}: agent step must have 'system_prompt' in config",
                    step_label
                ));
            }

            // Verify executor plugin binary exists
            if let Some(executor) = &step.executor {
                let plugin_path = format!("config/plugins/{}", executor);
                if !Path::new(&plugin_path).exists() {
                    issues.push(format!(
                        "{}: executor '{}' not found at {}",
                        step_label, executor, plugin_path
                    ));
                }
            }
        }
    }

    // Validate depends_on DAG: check references exist and detect cycles
    let step_ids: HashSet<String> = steps.iter().filter_map(|s| s.id.clone()).collect();

    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for step in steps {
        let id = match &step.id {
            Some(id) => id.clone(),
            None => continue,
        };
        adj.entry(id.clone()).or_default();

        if let Some(deps) = &step.depends_on {
            for dep in deps {
                if !step_ids.contains(dep) {
                    issues.push(format!(
                        "step '{}': depends_on references unknown step '{}'",
                        id, dep
                    ));
                }
            }
            adj.get_mut(&id).unwrap().extend(deps.clone());
        }

        // Validate context_from references
        if let Some(ctx) = &step.config.as_ref().and_then(|c| c.context_from.as_ref()) {
            for ref_id in *ctx {
                if !step_ids.contains(ref_id) {
                    issues.push(format!(
                        "step '{}': context_from references unknown step '{}'",
                        id, ref_id
                    ));
                }
            }
        }
    }

    // Cycle detection via DFS
    if has_cycle(&adj) {
        issues.push("depends_on graph contains a cycle".to_string());
    }

    // Validate required plugin dependencies exist
    if let Some(requires) = &workflow.requires {
        for req in requires {
            let plugin_path = format!("config/plugins/{}", req.plugin);
            if !Path::new(&plugin_path).exists() {
                issues.push(format!(
                    "requires plugin '{}' but not found at {}",
                    req.plugin, plugin_path
                ));
            }
        }
    }

    Ok(ValidationResult {
        file: path.display().to_string(),
        valid: issues.is_empty(),
        issues,
    })
}

#[derive(Debug)]
pub struct ValidationResult {
    pub file: String,
    pub valid: bool,
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn valid_workflow_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - type: agent\n    id: s1\n    config:\n      system_prompt: hello"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(result.valid, "issues: {:?}", result.issues);
    }

    #[test]
    fn missing_name_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "version: 1\nsteps:\n  - id: s1\n    type: function").unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("name")));
    }

    #[test]
    fn missing_steps_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "name: test\nversion: 1").unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("steps")));
    }

    #[test]
    fn invalid_yaml_reports_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "name: test\n  bad indent: [").unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("invalid YAML")));
    }

    #[test]
    fn agent_step_without_system_prompt_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - id: s1\n    type: agent\n    config:\n      max_tokens: 1024"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("system_prompt")));
    }

    #[test]
    fn empty_steps_array_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "name: test\nversion: 1\nsteps: []").unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("empty")));
    }

    #[test]
    fn step_missing_id_reports_issue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - type: function\n    config:\n      command: ['echo']"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("id")));
    }

    #[test]
    fn requires_missing_plugin_reports_issue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nrequires:\n  - plugin: nonexistent-plugin\nsteps:\n  - id: s1\n    type: function"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("nonexistent-plugin")));
    }

    #[test]
    fn depends_on_unknown_step_reports_issue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - id: s1\n    type: function\n    depends_on: [nonexistent]"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("depends_on") && i.contains("nonexistent")));
    }

    #[test]
    fn depends_on_cycle_reports_issue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - id: a\n    type: function\n    depends_on: [b]\n  - id: b\n    type: function\n    depends_on: [a]"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result.issues.iter().any(|i| i.contains("cycle")));
    }

    #[test]
    fn context_from_unknown_step_reports_issue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - id: s1\n    type: agent\n    config:\n      system_prompt: hello\n      context_from: [ghost]"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("context_from") && i.contains("ghost")));
    }

    #[test]
    fn valid_depends_on_dag_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "name: test\nversion: 1\nsteps:\n  - id: a\n    type: function\n    depends_on: []\n  - id: b\n    type: function\n    depends_on: [a]"
        )
        .unwrap();

        let result = validate_workflow_file(&path).unwrap();
        assert!(result.valid, "issues: {:?}", result.issues);
    }
}
