# Story 22.4: Auto-link PRs to Source GitHub Issues

Status: ready-for-dev

## Story

As an operator,
I want PRs created by auto-dev to automatically reference the originating GitHub Issue,
So that merging the PR closes the issue, completing the full Issue-to-Merge lifecycle.

## Acceptance Criteria

1. **Given** a board task has `issue_number` in its metadata (set by sync in Story 22.2), **When** the auto-dev workflow reaches the create-pr step, **Then** the PR body includes `Closes #N` where N is the issue number.

2. **Given** the executor builds `template_vars` for a workflow run, **When** the task metadata contains `issue_number`, **Then** `{{issue_number}}` is available as a template variable in all workflow steps, **And** `{{issue_url}}` is also available.

3. **Given** a task was manually created on the board (no `issue_number` metadata), **When** the PR is created, **Then** no `Closes #N` is added and the PR body is unchanged (graceful fallback).

4. **Given** existing workflow templates (`coding-feature-dev`, `coding-story-dev`, `coding-bug-fix`), **When** the create-pr step command is updated, **Then** the PR body template includes `{{issue_closing_ref}}` which resolves to `Closes #N` or empty string.

## Tasks / Subtasks

- [ ] Task 1: Inject issue metadata into template_vars in the auto-dev loop (AC: 1, 2)
  - [ ] 1.1 In `src/auto_dev.rs`, in the `auto_dev_next()` function, after picking a task, fetch its full metadata via `GET /api/v1/tasks/{id}`
  - [ ] 1.2 Extract `issue_number` and `issue_url` from the task metadata JSON
  - [ ] 1.3 Pass these values to the executor so they become available as template variables
  - [ ] 1.4 This requires modifying how `auto_dev_next()` calls `executor::execute_workflow_with_config()` -- the issue metadata must be injected into the template_vars before the workflow executes

- [ ] Task 2: Add issue template vars seeding in executor (AC: 2)
  - [ ] 2.1 In `src/executor.rs`, in `execute_workflow_with_config()`, the template_vars HashMap is built at line 135. Add a new function or parameter to seed additional template vars from task metadata
  - [ ] 2.2 Add a new public function: `pub fn execute_workflow_with_vars(workflow_id, user_input, config, extra_vars: HashMap<String, String>)` that accepts extra template variables
  - [ ] 2.3 The existing `execute_workflow_with_config()` can delegate to `execute_workflow_with_vars()` with an empty extra_vars map for backward compatibility
  - [ ] 2.4 In the new function, merge `extra_vars` into `template_vars` before executing steps
  - [ ] 2.5 Seed these template vars from issue metadata: `issue_number`, `issue_url`, `issue_closing_ref`

- [ ] Task 3: Build `issue_closing_ref` template variable (AC: 1, 3)
  - [ ] 3.1 If `issue_number` is present in metadata, set `issue_closing_ref` to `"Closes #N"` (where N is the issue number)
  - [ ] 3.2 If `issue_number` is NOT present (manually created task), set `issue_closing_ref` to empty string `""`
  - [ ] 3.3 This ensures `{{issue_closing_ref}}` in templates resolves gracefully to nothing when no issue exists

- [ ] Task 4: Modify `auto_dev_next()` to fetch and pass issue metadata (AC: 1, 2)
  - [ ] 4.1 In `src/auto_dev.rs`, after `pick_next_task()` returns an `Assignment`, fetch the task's full record via `pulse_api::get_task()` or a new `board_client` function that returns metadata
  - [ ] 4.2 Extract metadata fields: parse `issue_number` (as u64), `issue_url` (as String)
  - [ ] 4.3 Build `extra_vars: HashMap<String, String>` with: `issue_number`, `issue_url`, `issue_closing_ref`
  - [ ] 4.4 Call `executor::execute_workflow_with_vars()` instead of `execute_workflow_with_config()`
  - [ ] 4.5 If metadata fetch fails or fields are missing, proceed with empty extra_vars (graceful fallback -- no issue linking for this task)

- [ ] Task 5: Fetch task metadata helper (AC: 1, 2)
  - [ ] 5.1 Add `pub fn get_task_metadata(task_id: &str) -> Result<serde_json::Value, WitPluginError>` to `src/board_client.rs`
  - [ ] 5.2 GET `http://127.0.0.1:{port}/api/v1/tasks/{task_id}` and extract the `metadata` field from the response
  - [ ] 5.3 Return the metadata as a `serde_json::Value` (JSON object), or an empty object if no metadata exists
  - [ ] 5.4 Follow existing error handling pattern: `api_err(format!("GET {url}: {e}"))`

- [ ] Task 6: Update workflow YAML templates with `{{issue_closing_ref}}` (AC: 4)
  - [ ] 6.1 Update `config/workflows/coding-feature-dev.yaml` -- modify the `create_pr` step's command to append `{{issue_closing_ref}}` to the PR body
  - [ ] 6.2 Update `config/workflows/coding-story-dev.yaml` -- same modification
  - [ ] 6.3 Update `config/workflows/coding-bug-fix.yaml` -- same modification
  - [ ] 6.4 The `generate_pr_body` agent step in `coding-feature-dev.yaml` should include `{{issue_closing_ref}}` in its prompt template so the generated PR body includes the closing reference

- [ ] Task 7: Write unit tests (AC: 1, 2, 3)
  - [ ] 7.1 `test_issue_closing_ref_with_issue_number` -- verify `Closes #42` is generated
  - [ ] 7.2 `test_issue_closing_ref_without_issue_number` -- verify empty string for manual tasks
  - [ ] 7.3 `test_extra_vars_from_metadata_with_issue` -- verify all three vars are populated
  - [ ] 7.4 `test_extra_vars_from_metadata_without_issue` -- verify empty vars when no metadata
  - [ ] 7.5 `test_template_substitution_with_issue_vars` -- verify `{{issue_closing_ref}}` resolves in command strings

- [ ] Task 8: Write integration test (AC: 1, 4)
  - [ ] 8.1 Add `#[ignore]` test in `tests/github_sync_integration.rs`
  - [ ] 8.2 `test_auto_dev_pr_includes_closing_ref` -- create a task with issue_number metadata, run auto-dev, verify PR body contains `Closes #N`

## Dev Notes

### How Template Variables Work in the Executor (`src/executor.rs`)

The executor builds a `HashMap<String, String>` of template variables that are substituted into workflow step commands and prompts. The flow:

```rust
// src/executor.rs, execute_workflow_with_config() -- line 134
let mut template_vars: HashMap<String, String> = HashMap::new();
template_vars.insert("input".to_string(), user_input.to_string());
template_vars.insert("workflow_id".to_string(), workflow_id.to_string());
// ... default_model, max_budget_usd if configured ...

// These vars are used throughout workflow execution:
// - In function step commands: ["plugin-git-pr", "create-pr", "{{pr_title}}", "{{pr_body}}"]
// - In agent prompts: "Design the architecture for: {{input}}"
// - Vars are accumulated during execution: branch_name, pr_title, pr_body, working_dir, session_id
```

Template substitution uses `substitute_templates(text, &template_vars)` which replaces `{{key}}` with the value. If a key is not found, the `{{key}}` placeholder stays as-is (no error). This means `{{issue_closing_ref}}` will remain as the literal string if the var is not set, so you MUST set it to empty string for manual tasks.

### Approach: New Executor Entry Point

Add a new function that accepts extra template vars:

```rust
// src/executor.rs -- NEW function
pub fn execute_workflow_with_vars(
    workflow_id: &str,
    user_input: &str,
    config: &WorkspaceConfig,
    extra_vars: HashMap<String, String>,
) -> Result<serde_json::Value, WitPluginError> {
    // ... same setup as execute_workflow_with_config() ...
    let mut template_vars: HashMap<String, String> = HashMap::new();
    template_vars.insert("input".to_string(), user_input.to_string());
    template_vars.insert("workflow_id".to_string(), workflow_id.to_string());
    // ... default_model, max_budget_usd ...

    // Merge extra vars (issue metadata)
    for (k, v) in extra_vars {
        template_vars.insert(k, v);
    }

    // ... rest of workflow execution ...
}

// Make the existing function delegate:
pub fn execute_workflow_with_config(
    workflow_id: &str,
    user_input: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    execute_workflow_with_vars(workflow_id, user_input, config, HashMap::new())
}
```

### Auto-Dev Loop Modification (`src/auto_dev.rs`)

The `auto_dev_next()` function (line 131) currently calls the executor directly:

```rust
// Current code in auto_dev.rs:
let workflow_result = executor::execute_workflow_with_config(&workflow_id, &user_input, config);
```

Change to:

```rust
// Fetch task metadata for issue linking
let extra_vars = build_issue_template_vars(&task.id);

let workflow_result = executor::execute_workflow_with_vars(
    &workflow_id, &user_input, config, extra_vars
);
```

Add a helper function:

```rust
/// Build template vars from task metadata for issue linking.
/// Returns empty map if task has no issue metadata (graceful fallback).
fn build_issue_template_vars(task_id: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    let meta = match board_client::get_task_metadata(task_id) {
        Ok(m) => m,
        Err(_) => return vars, // graceful fallback
    };

    if let Some(number) = meta.get("issue_number").and_then(|v| v.as_u64()) {
        vars.insert("issue_number".to_string(), number.to_string());
        vars.insert(
            "issue_closing_ref".to_string(),
            format!("Closes #{}", number),
        );
    }

    if let Some(url) = meta.get("issue_url").and_then(|v| v.as_str()) {
        vars.insert("issue_url".to_string(), url.to_string());
    }

    // If no issue_number, set closing_ref to empty for clean template resolution
    if !vars.contains_key("issue_closing_ref") {
        vars.insert("issue_closing_ref".to_string(), String::new());
    }

    vars
}
```

### Task Metadata Retrieval

The task metadata is stored in the Pulse task record. The existing `pulse_api::get_task()` returns a `PulseTask` but it does NOT include metadata. You need a new function in `board_client.rs`:

```rust
/// Get task metadata as a JSON object.
pub fn get_task_metadata(task_id: &str) -> Result<serde_json::Value, WitPluginError> {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}/api/v1/tasks/{}", port, task_id);
    let body = reqwest::blocking::get(&url)
        .map_err(|e| api_err(format!("GET {url}: {e}")))?
        .text()
        .map_err(|e| api_err(e))?;
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;
    let task_val = val.get("task").unwrap_or(&val);
    Ok(task_val.get("metadata").cloned().unwrap_or(serde_json::json!({})))
}
```

This follows the same pattern used in `add_comment()` (line 106 of `board_client.rs`) which already fetches task data and reads metadata.

### Workflow YAML Template Updates

Three workflow files need their `create_pr` step updated to include `{{issue_closing_ref}}`:

**`config/workflows/coding-feature-dev.yaml`** (line 155):
```yaml
# Current:
command: ["plugin-git-pr", "create-pr", "{{pr_title}}", "{{pr_body}}"]
# Updated:
command: ["plugin-git-pr", "create-pr", "{{pr_title}}", "{{pr_body}}\n\n{{issue_closing_ref}}"]
```

Also update the `generate_pr_body` agent step (line 138) system prompt to include the closing ref:
```yaml
system_prompt: |
  Synthesize a pull request description from the provided workflow context.
  First line: PR title (max 72 chars, start with "feat: ").
  Then blank line, then PR body in markdown with sections: ## Summary, ## Changes, ## Test Results.
  If issue_closing_ref is provided, include it at the end of the body.
  End with: ---\nGenerated by Pulse Auto-Dev
user_prompt_template: "Generate PR description for: {{input}}\n\nIssue reference: {{issue_closing_ref}}"
```

**`config/workflows/coding-story-dev.yaml`** (line 158):
```yaml
# Current:
command: ["plugin-git-pr", "create-pr", "feat: {{input}}"]
# Updated:
command: ["plugin-git-pr", "create-pr", "feat: {{input}}", "{{issue_closing_ref}}"]
```

**`config/workflows/coding-bug-fix.yaml`** (line 126):
```yaml
# Current:
command: ["plugin-git-pr", "create-pr", "fix: {{input}}"]
# Updated:
command: ["plugin-git-pr", "create-pr", "fix: {{input}}", "{{issue_closing_ref}}"]
```

Note: When `{{issue_closing_ref}}` is empty string, the PR body/title will have trailing whitespace or an empty argument. Verify that `plugin-git-pr` handles empty trailing arguments gracefully (it likely does since it's a bash script).

### Graceful Fallback Behavior (AC: 3)

The key design principle is that **everything works when no issue metadata exists**:

1. `build_issue_template_vars()` returns empty HashMap on any error or missing metadata
2. `issue_closing_ref` is always set -- either `"Closes #N"` or `""` (empty string)
3. `{{issue_closing_ref}}` in templates resolves to empty string, producing clean PR bodies
4. If `execute_workflow_with_vars` receives empty extra_vars, behavior is identical to the current `execute_workflow_with_config`

### Files to Modify

- `src/executor.rs` -- add `execute_workflow_with_vars()`, refactor `execute_workflow_with_config()` to delegate
- `src/auto_dev.rs` -- modify `auto_dev_next()` to fetch metadata and call `execute_workflow_with_vars()`
- `src/board_client.rs` -- add `get_task_metadata()` function
- `config/workflows/coding-feature-dev.yaml` -- update `create_pr` and `generate_pr_body` steps
- `config/workflows/coding-story-dev.yaml` -- update `create_pr` step
- `config/workflows/coding-bug-fix.yaml` -- update `create_pr` step

### Error Handling Constraints

- All errors map to `WitPluginError` using `internal()` for API/network errors
- NEVER `unwrap()` or `expect()` in production code
- Metadata fetch failures must be GRACEFUL -- log a warning and proceed without issue linking
- NEVER log GITHUB_TOKEN or any credential values
- Use `tracing::debug!` for issue metadata injection, `tracing::warn!` for metadata fetch failures

### Anti-Patterns to Avoid

- **Do NOT** make issue metadata required -- tasks without metadata must work identically to current behavior
- **Do NOT** leave `{{issue_closing_ref}}` unset for manual tasks -- always set it to empty string to prevent literal `{{issue_closing_ref}}` appearing in PR bodies
- **Do NOT** modify `execute_workflow_with_config()` signature -- add a new function and delegate for backward compatibility
- **Do NOT** use async reqwest -- use `reqwest::blocking` only
- **Do NOT** use `unwrap()` or `expect()` in production code
- **Do NOT** use `println!` or `eprintln!` for logging -- use `tracing` macros
- **Do NOT** duplicate the existing executor setup code -- refactor to share between the two entry points

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests`):

In `src/auto_dev.rs`:
- Test `build_issue_template_vars()` with metadata containing `issue_number` and `issue_url`
- Test `build_issue_template_vars()` with empty metadata (graceful fallback)
- Test `build_issue_template_vars()` with partial metadata (only issue_number, no issue_url)

In `src/executor.rs`:
- Test that `execute_workflow_with_vars()` with empty extra_vars matches `execute_workflow_with_config()` behavior
- Test template substitution: `{{issue_closing_ref}}` resolves to `Closes #42` when set
- Test template substitution: `{{issue_closing_ref}}` resolves to empty string when set to `""`

**Integration tests** (`tests/github_sync_integration.rs` with `#[ignore]`):
- Full end-to-end: sync issue, run auto-dev, verify PR body contains `Closes #N`

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Story 22.4]
- [Source: src/executor.rs -- template_vars setup (line 134), substitute_templates(), execute_workflow_with_config()]
- [Source: src/auto_dev.rs -- auto_dev_next() (line 131), workflow execution call (line 158)]
- [Source: src/board_client.rs -- add_comment() metadata fetch pattern (line 106), update_assignment()]
- [Source: config/workflows/coding-feature-dev.yaml -- create_pr step (line 149), generate_pr_body (line 131)]
- [Source: config/workflows/coding-story-dev.yaml -- create_pr step (line 152)]
- [Source: config/workflows/coding-bug-fix.yaml -- create_pr step (line 120)]

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
