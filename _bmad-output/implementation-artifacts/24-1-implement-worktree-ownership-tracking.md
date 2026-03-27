# Story 24.1: Implement Worktree Ownership Tracking

Status: done

## Story

As a platform operator,
I want the system to track which task/workflow owns which git worktree,
So that worktrees can be safely cleaned up, conflicts detected, and status reported.

## Acceptance Criteria

1. **Given** a new `src/worktree_tracker.rs` module is created, **When** the `WorktreeRegistry` struct is defined, **Then** it tracks entries with: `worktree_path`, `branch_name`, `task_id`, `workflow_id`, `created_at`, `status` (active/completed/failed/orphaned), and the registry is persisted to `config/worktree-registry.json` for durability across process restarts.

2. **Given** a workflow creates a worktree (via `git worktree add`), **When** `register_worktree(path, branch, task_id, workflow_id)` is called, **Then** the entry is added to the registry with status `active` and current timestamp, and the registry file is updated atomically (write to temp file, then rename).

3. **Given** a workflow completes (success or failure), **When** `update_worktree_status(path, new_status)` is called, **Then** the entry's status is updated to `completed` or `failed` and the registry file is persisted atomically.

4. **Given** a registry file already exists from a prior process run, **When** the module loads the registry, **Then** the existing entries are preserved and new entries are appended.

5. **Given** `register_worktree()` is called with a path that already exists in the registry, **When** the call completes, **Then** the existing entry is updated (not duplicated) with the new task/workflow/timestamp.

6. **Given** the executor's `execute_workflow_with_config()` runs a workflow with a git worktree add step, **When** the worktree is created successfully, **Then** the executor automatically calls `register_worktree()` after the worktree step succeeds, and calls `update_worktree_status()` when the workflow completes or fails.

7. **Given** `cargo clippy -- -D warnings` is run, **When** the new module and executor changes compile, **Then** no warnings or errors are reported.

## Tasks / Subtasks

- [x] Task 1: Create `src/worktree_tracker.rs` module with types (AC: 1)
  - [x] 1.1 Define `WorktreeStatus` enum with variants: `Active`, `Completed`, `Failed`, `Orphaned`. Derive `Debug, Clone, Serialize, Deserialize, PartialEq`. Use `#[serde(rename_all = "lowercase")]` so JSON values are `"active"`, `"completed"`, `"failed"`, `"orphaned"`.
  - [x] 1.2 Define `WorktreeEntry` struct with fields: `worktree_path: String`, `branch_name: String`, `task_id: String`, `workflow_id: String`, `created_at: String` (ISO 8601 UTC), `status: WorktreeStatus`. Derive `Debug, Clone, Serialize, Deserialize`.
  - [x] 1.3 Define `WorktreeRegistry` struct with fields: `entries: Vec<WorktreeEntry>`. Derive `Debug, Clone, Serialize, Deserialize, Default`.
  - [x] 1.4 Add `#[cfg(not(target_arch = "wasm32"))] pub mod worktree_tracker;` to `src/lib.rs` (after `board_client` line).
  - [x] 1.5 Add `use crate::workspace::WorkspaceConfig;` and `use pulse_plugin_sdk::error::WitPluginError;` imports.

- [x] Task 2: Implement registry persistence functions (AC: 1, 2, 4)
  - [x] 2.1 Implement `fn registry_path(config: &WorkspaceConfig) -> PathBuf` — returns `config.base_dir.join("config/worktree-registry.json")`.
  - [x] 2.2 Implement `pub fn load_registry(config: &WorkspaceConfig) -> Result<WorktreeRegistry, WitPluginError>` — reads the JSON file if it exists, returns empty registry if file is missing. Use `std::fs::read_to_string` then `serde_json::from_str`. Map IO errors to `WitPluginError::internal()`.
  - [x] 2.3 Implement `fn save_registry(config: &WorkspaceConfig, registry: &WorktreeRegistry) -> Result<(), WitPluginError>` — atomic write: serialize to JSON with `serde_json::to_string_pretty`, write to `{path}.tmp`, then `std::fs::rename` to final path. Ensure parent directory exists with `std::fs::create_dir_all`. Map errors to `WitPluginError::internal()`.

- [x] Task 3: Implement `register_worktree()` function (AC: 2, 5)
  - [x] 3.1 Implement `pub fn register_worktree(config: &WorkspaceConfig, worktree_path: &str, branch_name: &str, task_id: &str, workflow_id: &str) -> Result<(), WitPluginError>`.
  - [x] 3.2 Load existing registry via `load_registry()`.
  - [x] 3.3 Check if an entry with the same `worktree_path` already exists. If yes, update its fields in place. If no, push a new `WorktreeEntry`.
  - [x] 3.4 Set `status` to `WorktreeStatus::Active` and `created_at` to current UTC time formatted as ISO 8601 string. Use `std::time::SystemTime::now()` and format manually (see Dev Notes for format helper).
  - [x] 3.5 Save registry via `save_registry()`.
  - [x] 3.6 Add `tracing::info!(plugin = "coding-pack", path = worktree_path, task = task_id, "registered worktree");` log.

- [x] Task 4: Implement `update_worktree_status()` function (AC: 3)
  - [x] 4.1 Implement `pub fn update_worktree_status(config: &WorkspaceConfig, worktree_path: &str, new_status: WorktreeStatus) -> Result<(), WitPluginError>`.
  - [x] 4.2 Load registry, find entry by `worktree_path`. If not found, return `WitPluginError::not_found()` with descriptive message.
  - [x] 4.3 Update the entry's `status` field.
  - [x] 4.4 Save registry atomically.
  - [x] 4.5 Add `tracing::info!(plugin = "coding-pack", path = worktree_path, status = ?new_status, "updated worktree status");` log.

- [x] Task 5: Integrate with executor (AC: 6)
  - [x] 5.1 In `src/executor.rs`, add `use crate::worktree_tracker;` import (behind `#[cfg(not(target_arch = "wasm32"))]`).
  - [x] 5.2 In `execute_workflow_with_config()`, after the existing `extract_worktree_path()` block that sets `template_vars["working_dir"]`, add a call to `worktree_tracker::register_worktree()`. Extract the branch name from the git command arguments or from `template_vars`. If registration fails, log a warning but do NOT fail the workflow.
  - [x] 5.3 At the end of `execute_workflow_with_config()`, in both the success and error paths, call `worktree_tracker::update_worktree_status()` with `Completed` or `Failed` respectively. Only call this if a worktree was registered during this run (track with a local `Option<String>` for the worktree path). If the status update fails, log a warning but do NOT fail the workflow.
  - [x] 5.4 Gate all worktree_tracker calls with `#[cfg(not(target_arch = "wasm32"))]` blocks.

- [x] Task 6: Write unit tests (AC: 1, 2, 3, 4, 5)
  - [x] 6.1 `test_worktree_status_serialization` — verify `WorktreeStatus::Active` serializes to `"active"` and round-trips correctly.
  - [x] 6.2 `test_worktree_entry_serialization` — create a `WorktreeEntry`, serialize to JSON, deserialize back, verify all fields match.
  - [x] 6.3 `test_load_missing_registry_returns_empty` — call `load_registry()` with a config pointing to a nonexistent directory, verify empty entries.
  - [x] 6.4 `test_register_and_load_round_trip` — register a worktree, then load registry and verify the entry is present with status `Active`.
  - [x] 6.5 `test_register_duplicate_path_updates` — register same path twice with different task_ids, verify only one entry exists with the second task_id.
  - [x] 6.6 `test_update_status_changes_entry` — register a worktree, then update status to `Completed`, verify the change persists.
  - [x] 6.7 `test_update_status_not_found` — call `update_worktree_status()` for a nonexistent path, verify `not_found` error.
  - [x] 6.8 `test_atomic_write_creates_file` — verify `save_registry()` creates the file and no `.tmp` file remains after save.
  - [x] 6.9 Use `tempfile::tempdir()` for all tests that write to disk. Add `tempfile` as a dev-dependency in `Cargo.toml` if not already present.

## Dev Notes

### Registry File Location and Format

The registry file lives at `{workspace_base_dir}/config/worktree-registry.json`. Example content:

```json
{
  "entries": [
    {
      "worktree_path": "/home/user/project/.worktrees/auto-dev/coding-story-dev/task-42",
      "branch_name": "auto-dev/coding-story-dev/task-42",
      "task_id": "task-42",
      "workflow_id": "coding-story-dev",
      "created_at": "2026-03-27T14:30:00Z",
      "status": "active"
    },
    {
      "worktree_path": "/home/user/project/.worktrees/auto-dev/coding-bug-fix/task-99",
      "branch_name": "auto-dev/coding-bug-fix/task-99",
      "task_id": "task-99",
      "workflow_id": "coding-bug-fix",
      "created_at": "2026-03-27T12:00:00Z",
      "status": "completed"
    }
  ]
}
```

### Atomic File Write Pattern

This pattern is used throughout the codebase for safe JSON persistence. The critical sequence is: write to temp file, then rename. This ensures that a crash during write never corrupts the registry.

```rust
use std::io::Write;

fn save_registry(config: &WorkspaceConfig, registry: &WorktreeRegistry) -> Result<(), WitPluginError> {
    let path = registry_path(config);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WitPluginError::internal(format!("cannot create config dir: {e}")))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .map_err(|e| WitPluginError::internal(format!("JSON serialize error: {e}")))?;
    let tmp_path = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| WitPluginError::internal(format!("cannot create temp file: {e}")))?;
    file.write_all(json.as_bytes())
        .map_err(|e| WitPluginError::internal(format!("cannot write temp file: {e}")))?;
    file.sync_all()
        .map_err(|e| WitPluginError::internal(format!("cannot sync temp file: {e}")))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| WitPluginError::internal(format!("cannot rename temp to final: {e}")))?;
    Ok(())
}
```

### ISO 8601 Timestamp Without External Crate

Do NOT add `chrono` as a dependency. Use `std::time::SystemTime` and manual formatting:

```rust
fn now_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple UTC format: seconds since epoch to ISO 8601
    // For a rough but correct approach:
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    // Use a simple loop — this runs once per registration, performance is irrelevant
    let mut year = 1970i32;
    let mut remaining_days = days as i32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let days_in_months: [i32; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &d in &days_in_months {
        if remaining_days < d {
            break;
        }
        remaining_days -= d;
        month += 1;
    }
    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
```

Alternatively, you may use the simpler approach of storing the Unix timestamp as an integer and formatting it only for display purposes. Either way, do NOT add the `chrono` or `time` crate.

### Executor Integration Point

The key integration point is in `src/executor.rs` inside `execute_workflow_with_config()`. The existing code at approximately line 276-283 already detects worktree creation:

```rust
// Extract worktree path from git worktree add commands
if output.status == StepStatus::Success {
    if let Some(config) = &step.config {
        if let Some(cmd) = &config.command {
            if let Some(wt_path) = extract_worktree_path(cmd, template_vars) {
                eprintln!("[workflow]   worktree: {}", wt_path);
                template_vars.insert("working_dir".to_string(), wt_path);
            }
        }
    }
}
```

Add worktree registration immediately after the `template_vars.insert("working_dir", wt_path)` line. Extract the branch name from the git command arguments. The branch is typically the second-to-last argument of `git worktree add -b <branch> <path>`, or parse it from the `auto-dev/{workflow_id}/{task_id}` path convention.

To track the worktree path for final status update, add a local variable before the step loop:

```rust
let mut registered_worktree_path: Option<String> = None;
```

Set it when registration succeeds. Then at the function's exit points, call `update_worktree_status()` if the variable is `Some`.

### Branch Name Extraction from Git Command

The workflow YAML typically has commands like:
```yaml
command: ["git", "worktree", "add", "-b", "auto-dev/{{workflow_id}}/{{task_id}}", ".worktrees/auto-dev/{{workflow_id}}/{{task_id}}"]
```

To extract the branch name, look for the `-b` flag in the resolved command args:

```rust
fn extract_branch_name(command: &[String], template_vars: &HashMap<String, String>) -> Option<String> {
    let resolved: Vec<String> = command
        .iter()
        .map(|part| substitute_templates(part, template_vars))
        .collect();
    let mut iter = resolved.iter();
    while let Some(arg) = iter.next() {
        if arg == "-b" || arg == "-B" {
            return iter.next().cloned();
        }
    }
    None
}
```

If `-b` is not found, fall back to deriving the branch from the worktree path by stripping the `.worktrees/` prefix.

### Error Handling in Executor Integration

Registration and status update failures must NOT cause the workflow to fail. Wrap calls in a match and log warnings:

```rust
#[cfg(not(target_arch = "wasm32"))]
{
    if let Err(e) = worktree_tracker::register_worktree(config, &wt_path, &branch, &task_id, &workflow_id) {
        tracing::warn!(plugin = "coding-pack", error = %e, "failed to register worktree — continuing workflow");
    }
}
```

### Dependencies

All required dependencies exist in `Cargo.toml`:
- `serde = { version = "1.0", features = ["derive"] }` — serialization
- `serde_json = "1.0"` — JSON persistence
- `tracing = "0.1"` — structured logging
- `pulse-plugin-sdk` — `WitPluginError`

For tests, you may need `tempfile` as a dev-dependency. Check `Cargo.toml` first — if absent, add `tempfile = "3"` under `[dev-dependencies]`.

**No new production dependencies needed.**

### WASM Gate

The entire `worktree_tracker` module must be behind `#[cfg(not(target_arch = "wasm32"))]` because it uses filesystem I/O and `std::process::Command` is not available on WASM. This matches the pattern used by `board_client`, `config_injector`, `agent_registry`, and `tool_provider`.

### Module File Placement

- **New file**: `src/worktree_tracker.rs` — flat module in `src/`, no nested directories
- **Modified files**: `src/lib.rs` (add module declaration), `src/executor.rs` (add registration/status calls)
- **No changes to**: `Cargo.toml` (unless `tempfile` dev-dep is missing), `src/workspace.rs`, `src/pack.rs` (pack action dispatch comes in Story 24.2)

### Anti-Patterns to Avoid

- **Do NOT** use `unwrap()` or `expect()` in production code — all errors map to `WitPluginError`
- **Do NOT** use `HashMap` for serialized output — use `Vec<WorktreeEntry>` for predictable ordering
- **Do NOT** use `println!` or `eprintln!` for logging — always use `tracing` macros
- **Do NOT** add `chrono` or `time` crate — use `std::time::SystemTime` for timestamps
- **Do NOT** lock the registry file — the auto-dev loop is single-threaded per-process; file locking adds complexity without benefit here
- **Do NOT** fail the workflow if worktree tracking fails — log a warning and continue
- **Do NOT** use async — all I/O is `std::fs` and `std::process::Command` (synchronous)
- **Do NOT** write to the registry path directly — always use the atomic temp+rename pattern
- **Do NOT** store absolute paths from a different machine — `worktree_path` should be the value from the git command, which may be relative or absolute depending on the workflow YAML

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests` in `worktree_tracker.rs`):
- All tests use `tempfile::tempdir()` for isolation
- Create a `WorkspaceConfig` pointing to the temp dir for each test
- Test serialization round-trips for all types
- Test register, update, load lifecycle
- Test duplicate path handling
- Test error cases (missing entry, corrupt file)

**Integration with executor** (tested in Story 24.2+ or manually):
- The executor integration is best tested by running a workflow end-to-end
- Unit tests for the executor changes are not required in this story — the registration calls are guarded by warning-only error handling

### Logging Conventions

```rust
tracing::info!(plugin = "coding-pack", path = %worktree_path, task = %task_id, "registered worktree");
tracing::info!(plugin = "coding-pack", path = %worktree_path, status = ?new_status, "updated worktree status");
tracing::warn!(plugin = "coding-pack", error = %e, "failed to register worktree — continuing workflow");
tracing::debug!(plugin = "coding-pack", entries = registry.entries.len(), "loaded worktree registry");
```

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Epic 24, Story 24.1]
- [Source: src/executor.rs — worktree extraction at line ~276-283, `extract_worktree_path()` at line ~648-670]
- [Source: src/board_client.rs — error helper pattern `fn api_err()`]
- [Source: src/workspace.rs — `WorkspaceConfig` with `base_dir` for path resolution]
- [Source: src/lib.rs — WASM gate pattern `#[cfg(not(target_arch = "wasm32"))]`]

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
- All 10 unit tests pass (worktree_tracker::tests::*)
- All 163 lib tests pass with no regressions
- All 32+ integration/e2e tests pass
- Clippy clean for worktree_tracker and executor changes (pre-existing github_sync dead_code warning)

### Completion Notes List
- Created `src/worktree_tracker.rs` with WorktreeStatus, WorktreeEntry, WorktreeRegistry types
- Implemented registry persistence with atomic write (temp + rename) pattern
- Implemented register_worktree() with duplicate-path update semantics
- Implemented update_worktree_status() with not_found error for missing entries
- Implemented now_iso8601() and is_leap_year() helpers without chrono dependency
- Integrated with executor: register on worktree creation, update status on workflow completion/failure
- Added extract_branch_from_command() helper for -b flag extraction
- All worktree_tracker calls gated with #[cfg(not(target_arch = "wasm32"))]
- Registration/status failures log warnings but do not fail the workflow

### File List
- `src/worktree_tracker.rs` (new) - worktree ownership tracking module
- `src/lib.rs` (modified) - added worktree_tracker module declaration behind WASM gate
- `src/executor.rs` (modified) - integrated worktree registration and status updates
