use std::path::Path;

/// Validate that a workflow YAML file has required structure.
pub fn validate_workflow_file(path: &Path) -> Result<ValidationResult, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;

    let mut issues = Vec::new();

    // Check required top-level fields
    if !content.contains("name:") {
        issues.push("missing 'name' field".to_string());
    }
    if !content.contains("version:") {
        issues.push("missing 'version' field".to_string());
    }
    if !content.contains("steps:") {
        issues.push("missing 'steps' field".to_string());
    }

    // Check that all agent steps have system_prompt
    let has_agent_steps = content.contains("type: agent");
    if has_agent_steps && !content.contains("system_prompt:") {
        issues.push("agent steps must have 'system_prompt'".to_string());
    }

    // Check for executor declarations on agent steps using plugin executors
    let expected_executors = ["bmad-method", "provider-claude-code", "plugin-git-worktree"];
    for executor in &expected_executors {
        if content.contains(&format!("executor: {}", executor)) {
            // Verify the plugin binary exists
            let plugin_path = format!("config/plugins/{}", executor);
            if !Path::new(&plugin_path).exists() {
                issues.push(format!(
                    "workflow references executor '{}' but plugin not found at {}",
                    executor, plugin_path
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
        writeln!(f, "version: 1\nsteps:\n  - id: s1").unwrap();

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
}
