//! E2E test harness for workflow executor integration testing.
//!
//! Provides helpers for setting up test fixtures, running workflows,
//! and asserting step outcomes.

use std::path::{Path, PathBuf};

/// E2E test harness that manages a temporary workspace for integration tests.
pub struct E2EHarness {
    /// Temporary directory containing a copy of the fixture project
    pub work_dir: PathBuf,
    /// Whether the temp dir should be cleaned up on drop
    cleanup: bool,
}

impl E2EHarness {
    /// Create a new harness by copying the fixture project to a temp directory.
    pub fn new(fixture_path: &Path) -> Result<Self, String> {
        let temp_dir = std::env::temp_dir().join(format!(
            "pulse-e2e-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        // Copy fixture to temp dir
        copy_dir_recursive(fixture_path, &temp_dir)
            .map_err(|e| format!("failed to copy fixture: {}", e))?;

        // Initialize git repo if not already
        if !temp_dir.join(".git").exists() {
            run_cmd(&temp_dir, "git", &["init"])?;
            run_cmd(&temp_dir, "git", &["add", "-A"])?;
            run_cmd(
                &temp_dir,
                "git",
                &["commit", "-m", "initial fixture commit"],
            )?;
        }

        Ok(Self {
            work_dir: temp_dir,
            cleanup: true,
        })
    }

    /// Submit a workflow and return the execution result JSON.
    pub fn submit_workflow(
        &self,
        workflow_id: &str,
        input: &str,
    ) -> Result<serde_json::Value, String> {
        // Use the project's executor directly
        plugin_coding_pack::executor::execute_workflow(workflow_id, input)
            .map_err(|e| format!("workflow execution failed: {}", e.message))
    }

    /// Assert that a step in the result has the expected status.
    pub fn assert_step_status(
        result: &serde_json::Value,
        step_id: &str,
        expected_status: &str,
    ) -> Result<(), String> {
        let steps = result["steps"]
            .as_array()
            .ok_or("no steps array in result")?;

        let step = steps
            .iter()
            .find(|s| s["step_id"].as_str() == Some(step_id))
            .ok_or_else(|| format!("step '{}' not found in result", step_id))?;

        let actual = step["status"].as_str().ok_or("step has no status field")?;

        if actual != expected_status {
            return Err(format!(
                "step '{}': expected status '{}', got '{}'",
                step_id, expected_status, actual
            ));
        }
        Ok(())
    }

    /// Read the content output of a specific step.
    pub fn read_step_output(result: &serde_json::Value, step_id: &str) -> Option<String> {
        result["steps"]
            .as_array()?
            .iter()
            .find(|s| s["step_id"].as_str() == Some(step_id))?
            .get("content_preview")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
    }
}

impl Drop for E2EHarness {
    fn drop(&mut self) {
        if self.cleanup && self.work_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.work_dir);
        }
    }
}

// ── Helper functions ───────────────────────────────────────────────────────

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn run_cmd(dir: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .map_err(|e| format!("failed to run {} {:?}: {}", program, args, e))?;

    if !output.status.success() {
        return Err(format!(
            "{} {:?} failed: {}",
            program,
            args,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// ── Tests for the harness itself ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_creates_temp_dir() {
        let fixture = Path::new("tests/fixtures/sample-project");
        if !fixture.exists() {
            return; // Skip if fixture not available
        }
        let harness = E2EHarness::new(fixture).unwrap();
        assert!(harness.work_dir.exists());
        assert!(harness.work_dir.join("Cargo.toml").exists());
        assert!(harness.work_dir.join("src/lib.rs").exists());
        assert!(harness.work_dir.join(".git").exists());
    }

    #[test]
    fn harness_cleans_up_on_drop() {
        let fixture = Path::new("tests/fixtures/sample-project");
        if !fixture.exists() {
            return;
        }
        let path;
        {
            let harness = E2EHarness::new(fixture).unwrap();
            path = harness.work_dir.clone();
            assert!(path.exists());
        }
        // After drop, temp dir should be cleaned up
        assert!(!path.exists());
    }

    #[test]
    fn assert_step_status_works() {
        let result = serde_json::json!({
            "steps": [
                {"step_id": "plan", "status": "success"},
                {"step_id": "implement", "status": "failed", "error": "timeout"},
            ]
        });

        assert!(E2EHarness::assert_step_status(&result, "plan", "success").is_ok());
        assert!(E2EHarness::assert_step_status(&result, "implement", "failed").is_ok());
        assert!(E2EHarness::assert_step_status(&result, "plan", "failed").is_err());
        assert!(E2EHarness::assert_step_status(&result, "nonexistent", "success").is_err());
    }

    #[test]
    fn read_step_output_extracts_content() {
        let result = serde_json::json!({
            "steps": [
                {"step_id": "plan", "status": "success", "content_preview": "design the system"},
                {"step_id": "implement", "status": "success"},
            ]
        });

        assert_eq!(
            E2EHarness::read_step_output(&result, "plan"),
            Some("design the system".to_string())
        );
        assert_eq!(E2EHarness::read_step_output(&result, "implement"), None);
        assert_eq!(E2EHarness::read_step_output(&result, "nonexistent"), None);
    }
}
