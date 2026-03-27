# Story 24.3: Recover Orphaned Worktrees and Report Status

Status: done

## Story

As a platform operator,
I want the system to detect and recover orphaned worktrees from failed workflows, and report overall worktree status,
So that I have visibility into worktree usage and can reclaim resources from abandoned runs.

## Acceptance Criteria

1. **Given** a worktree entry has status `failed` and was created more than 1 hour ago, **When** `recover_orphaned_worktrees()` is called, **Then** the entry is marked `orphaned` and then cleaned up (worktree directory removed via `git worktree remove`, branch deleted via `git branch -d`, entry removed from registry).

2. **Given** a worktree entry has status `failed` and was created less than 1 hour ago, **When** `recover_orphaned_worktrees()` is called, **Then** the entry is skipped (it may still be retried by the auto-dev loop).

3. **Given** `git worktree prune` can clean stale worktree metadata, **When** `recover_orphaned_worktrees()` runs, **Then** `git worktree prune` is called first before any other recovery operations.

4. **Given** `git worktree list` shows worktrees that are not in the registry, **When** `detect_untracked_worktrees()` is called, **Then** untracked worktrees with branches matching the `auto-dev/` prefix are added to the registry as `orphaned` entries, and worktrees without the `auto-dev/` prefix are ignored (they belong to the user).

5. **Given** a new action `worktree-status` is added to `execute_action()` in `src/pack.rs`, **When** invoked with `{"action": "worktree-status"}`, **Then** the result includes a JSON array of all tracked worktrees with: `path`, `branch`, `task_id`, `status`, `age` (human-readable, e.g., "2h 15m").

6. **Given** a new action `recover-worktrees` is added to `execute_action()` in `src/pack.rs`, **When** invoked with `{"action": "recover-worktrees"}`, **Then** the full recovery pipeline runs: `git worktree prune`, detect untracked worktrees, mark old failed entries as orphaned, clean up all orphaned entries, and return a result with counts.

7. **Given** the worktree status query operates on up to 20 active worktrees, **When** timing is measured, **Then** the query returns within 2 seconds (NFR-AD-5).

8. **Given** `cargo clippy -- -D warnings` is run, **When** all new functions and pack actions compile, **Then** no warnings or errors are reported.

## Tasks / Subtasks

- [x] Task 1: Implement `git worktree prune` and `git worktree list` helpers (AC: 3, 4)
  - [x] 1.1 Implement `fn run_git_worktree_prune(config: &WorkspaceConfig) -> Result<(), WitPluginError>` — runs `git worktree prune` via `std::process::Command` with `current_dir` set to `config.base_dir`. Log the result. If the command fails, log a warning and return `Ok(())` (prune failure is non-fatal).
  - [x] 1.2 Implement `fn list_git_worktrees(config: &WorkspaceConfig) -> Result<Vec<GitWorktreeInfo>, WitPluginError>` — runs `git worktree list --porcelain` via `std::process::Command`, parses output into structured data. Map command failures to `WitPluginError::internal()`.
  - [x] 1.3 Define `GitWorktreeInfo` struct: `path: String`, `branch: Option<String>`, `head: String`, `bare: bool`. This is an internal type, not serialized to the registry.

- [x] Task 2: Implement `detect_untracked_worktrees()` (AC: 4)
  - [x] 2.1 Implement `pub fn detect_untracked_worktrees(config: &WorkspaceConfig) -> Result<u32, WitPluginError>` — returns count of newly added orphaned entries.
  - [x] 2.2 Call `list_git_worktrees()` to get all worktrees known to git.
  - [x] 2.3 Load registry via `load_registry()`.
  - [x] 2.4 For each git worktree, check if its `path` already exists in the registry. If not, and if its branch matches the `auto-dev/` prefix, add it as a new `WorktreeEntry` with status `Orphaned`, `task_id` and `workflow_id` extracted from the branch name, and `created_at` set to current time.
  - [x] 2.5 Ignore worktrees whose branch does NOT start with `auto-dev/` — these belong to the user or other tools.
  - [x] 2.6 Ignore the main worktree (the bare repo itself, which has no branch prefix or is on `main`/`master`).
  - [x] 2.7 Save updated registry if any new entries were added.
  - [x] 2.8 Log: `tracing::info!(plugin = "coding-pack", detected = count, "detected untracked auto-dev worktrees");`

- [x] Task 3: Implement `recover_orphaned_worktrees()` (AC: 1, 2, 3)
  - [x] 3.1 Implement `pub fn recover_orphaned_worktrees(config: &WorkspaceConfig) -> Result<RecoveryResult, WitPluginError>`.
  - [x] 3.2 Define `RecoveryResult` struct: `pruned: bool`, `detected_untracked: u32`, `marked_orphaned: u32`, `cleaned: u32`, `skipped_recent: u32`, `errors: u32`. Derive `Debug, Serialize`.
  - [x] 3.3 Step 1: Call `run_git_worktree_prune()`.
  - [x] 3.4 Step 2: Call `detect_untracked_worktrees()` to find unregistered auto-dev worktrees.
  - [x] 3.5 Step 3: Load registry. For each entry with status `Failed`, compute age. If age > 1 hour, update status to `Orphaned`. Increment `marked_orphaned` counter. If age <= 1 hour, increment `skipped_recent` counter.
  - [x] 3.6 Step 4: For each entry with status `Orphaned`, call `cleanup_single_worktree()` (from Story 24.2). Track `cleaned` and `errors` counts.
  - [x] 3.7 Step 5: Rebuild registry entries — keep only entries that were NOT successfully cleaned. Save atomically.
  - [x] 3.8 Return `RecoveryResult` with all counts.
  - [x] 3.9 Log: `tracing::info!(plugin = "coding-pack", cleaned = result.cleaned, errors = result.errors, "orphaned worktree recovery complete");`

- [x] Task 4: Implement `worktree_status()` for status reporting (AC: 5, 7)
  - [x] 4.1 Implement `pub fn worktree_status(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError>`.
  - [x] 4.2 Load registry. For each entry, compute `age` as a human-readable string (e.g., "2h 15m", "5m", "3d 1h").
  - [x] 4.3 Build a JSON array of objects with fields: `path`, `branch`, `task_id`, `workflow_id`, `status`, `age`, `created_at`.
  - [x] 4.4 Return the JSON value: `{"worktrees": [...], "total": N, "by_status": {"active": 2, "completed": 1, ...}}`.
  - [x] 4.5 Implement `fn format_age(created_at: &str) -> String` — parse the ISO 8601 timestamp, compute duration from now, format as human-readable. If parsing fails, return `"unknown"`.

- [x] Task 5: Add pack actions `worktree-status` and `recover-worktrees` (AC: 5, 6)
  - [x] 5.1 In `src/pack.rs`, add match arm for `"worktree-status"` in `execute_action()`. Call `crate::worktree_tracker::worktree_status(&config)` and return via `to_json_string()`.
  - [x] 5.2 Add match arm for `"recover-worktrees"`. Call `crate::worktree_tracker::recover_orphaned_worktrees(&config)` and serialize the result.
  - [x] 5.3 Gate both actions with `#[cfg(not(target_arch = "wasm32"))]`. For WASM, return `WitPluginError::internal("not available in WASM")`.
  - [x] 5.4 Update the `other =>` catch-all error message to include `worktree-status`, `recover-worktrees`, and `cleanup-worktrees` in the available actions list.

- [x] Task 6: Write unit tests (AC: 1, 2, 4, 5, 7)
  - [x] 6.1 `test_parse_git_worktree_list_porcelain` — provide sample `git worktree list --porcelain` output, verify parsing into `GitWorktreeInfo` structs.
  - [x] 6.2 `test_detect_untracked_ignores_non_autodev_branches` — mock worktree list with `main` and `feature/foo` branches, verify they are not added to registry.
  - [x] 6.3 `test_detect_untracked_adds_autodev_branches` — mock worktree list with `auto-dev/coding-story-dev/task-42` branch, verify it is added as orphaned.
  - [x] 6.4 `test_recover_marks_old_failed_as_orphaned` — register a `Failed` entry with `created_at` 2 hours ago, run `recover_orphaned_worktrees()`, verify entry was marked orphaned and cleaned.
  - [x] 6.5 `test_recover_skips_recent_failed` — register a `Failed` entry with `created_at` 5 minutes ago, run recovery, verify entry is still in registry with status `Failed`.
  - [x] 6.6 `test_format_age_hours_minutes` — verify `format_age()` returns "2h 15m" for a timestamp 2 hours 15 minutes old.
  - [x] 6.7 `test_format_age_minutes_only` — verify `format_age()` returns "5m" for a timestamp 5 minutes old.
  - [x] 6.8 `test_format_age_days` — verify `format_age()` returns "3d 1h" for a timestamp 3 days 1 hour old.
  - [x] 6.9 `test_worktree_status_returns_json` — register entries with mixed statuses, call `worktree_status()`, verify JSON structure has `worktrees` array, `total`, `by_status`.
  - [x] 6.10 `test_worktree_status_action_dispatch` — call `execute_action()` with `{"action": "worktree-status"}`, verify valid JSON response.
  - [x] 6.11 `test_recover_worktrees_action_dispatch` — call `execute_action()` with `{"action": "recover-worktrees"}`, verify valid JSON response.
  - [x] 6.12 `test_extract_task_and_workflow_from_branch` — verify branch `auto-dev/coding-story-dev/task-42` extracts `workflow_id = "coding-story-dev"` and `task_id = "task-42"`.
  - [x] 6.13 Use `tempfile::tempdir()` for all tests that write to disk.

## Dev Notes

### Parsing `git worktree list --porcelain` Output

The `--porcelain` flag produces machine-readable output. Each worktree is a block of lines separated by a blank line:

```
worktree /home/user/project
HEAD abc123def456
branch refs/heads/main

worktree /home/user/project/.worktrees/auto-dev/coding-story-dev/task-42
HEAD def456abc789
branch refs/heads/auto-dev/coding-story-dev/task-42

worktree /home/user/project/.worktrees/feature/my-thing
HEAD 789abc123def
branch refs/heads/feature/my-thing
```

Parse this output line by line. Each block starts with `worktree <path>` and contains `HEAD <hash>` and `branch <refname>` (or `detached` if HEAD is detached).

```rust
fn list_git_worktrees(config: &WorkspaceConfig) -> Result<Vec<GitWorktreeInfo>, WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git worktree list: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WitPluginError::internal(format!("git worktree list failed: {}", stderr.trim())));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_head = String::new();
    let mut current_branch: Option<String> = None;
    let mut current_bare = false;

    for line in stdout.lines() {
        if line.is_empty() {
            if !current_path.is_empty() {
                worktrees.push(GitWorktreeInfo {
                    path: std::mem::take(&mut current_path),
                    head: std::mem::take(&mut current_head),
                    branch: current_branch.take(),
                    bare: current_bare,
                });
                current_bare = false;
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            current_head = head.to_string();
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            // branch_ref is like "refs/heads/auto-dev/coding-story-dev/task-42"
            // Strip "refs/heads/" prefix to get the branch name
            let branch_name = branch_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(branch_ref);
            current_branch = Some(branch_name.to_string());
        } else if line == "bare" {
            current_bare = true;
        }
        // "detached" line means HEAD is detached — current_branch stays None
    }
    // Handle last block (no trailing blank line)
    if !current_path.is_empty() {
        worktrees.push(GitWorktreeInfo {
            path: current_path,
            head: current_head,
            branch: current_branch,
            bare: current_bare,
        });
    }

    Ok(worktrees)
}
```

### Extracting Task ID and Workflow ID from Branch Name

The branch naming convention is `auto-dev/{workflow_id}/{task_id}`. Extract these parts:

```rust
fn extract_ids_from_branch(branch: &str) -> Option<(String, String)> {
    let stripped = branch.strip_prefix("auto-dev/")?;
    let slash_pos = stripped.find('/')?;
    let workflow_id = stripped[..slash_pos].to_string();
    let task_id = stripped[slash_pos + 1..].to_string();
    if workflow_id.is_empty() || task_id.is_empty() {
        return None;
    }
    Some((workflow_id, task_id))
}
```

### Age Calculation Without External Crate

Parse the ISO 8601 timestamp from the registry and compute age against current time. Both timestamps are in UTC seconds since epoch.

```rust
fn parse_iso8601_to_epoch(ts: &str) -> Option<u64> {
    // Expected format: "2026-03-27T14:30:00Z"
    // Parse manually — no chrono dependency
    let parts: Vec<&str> = ts.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<u32> = time_str.split(':').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }
    let (year, month, day) = (date_parts[0] as i32, date_parts[1], date_parts[2]);
    let (hours, minutes, seconds) = (time_parts[0], time_parts[1], time_parts[2]);

    // Days from epoch (1970-01-01) to target date
    let mut total_days: i64 = 0;
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }
    let days_in_months: [u32; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    for m in 0..(month as usize - 1) {
        total_days += days_in_months[m] as i64;
    }
    total_days += (day - 1) as i64;

    let total_secs = total_days * 86400 + hours as i64 * 3600 + minutes as i64 * 60 + seconds as i64;
    Some(total_secs as u64)
}

fn format_age(created_at: &str) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let created_secs = match parse_iso8601_to_epoch(created_at) {
        Some(s) => s,
        None => return "unknown".to_string(),
    };

    if now_secs < created_secs {
        return "0m".to_string();
    }

    let diff = now_secs - created_secs;
    let days = diff / 86400;
    let hours = (diff % 86400) / 3600;
    let minutes = (diff % 3600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
```

Share `is_leap_year()` with Story 24.1's `now_iso8601()` function. If both are in the same module, just define it once.

### Age Threshold for Orphan Detection

The 1-hour threshold is calculated as: if `now_epoch - created_epoch > 3600`, the entry is eligible for orphan marking. This gives the auto-dev retry loop time to attempt retries before the entry is cleaned up.

```rust
const ORPHAN_AGE_THRESHOLD_SECS: u64 = 3600; // 1 hour

fn is_old_enough_for_orphan(created_at: &str) -> bool {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match parse_iso8601_to_epoch(created_at) {
        Some(created_secs) => now_secs.saturating_sub(created_secs) > ORPHAN_AGE_THRESHOLD_SECS,
        None => true, // If we can't parse the timestamp, treat as old
    }
}
```

### Expected JSON Output for `worktree-status`

```json
{
  "worktrees": [
    {
      "path": "/home/user/project/.worktrees/auto-dev/coding-story-dev/task-42",
      "branch": "auto-dev/coding-story-dev/task-42",
      "task_id": "task-42",
      "workflow_id": "coding-story-dev",
      "status": "active",
      "age": "2h 15m",
      "created_at": "2026-03-27T14:30:00Z"
    },
    {
      "path": "/home/user/project/.worktrees/auto-dev/coding-bug-fix/task-99",
      "branch": "auto-dev/coding-bug-fix/task-99",
      "task_id": "task-99",
      "workflow_id": "coding-bug-fix",
      "status": "failed",
      "age": "45m",
      "created_at": "2026-03-27T16:00:00Z"
    }
  ],
  "total": 2,
  "by_status": {
    "active": 1,
    "failed": 1
  }
}
```

### Expected JSON Output for `recover-worktrees`

```json
{
  "pruned": true,
  "detected_untracked": 1,
  "marked_orphaned": 2,
  "cleaned": 3,
  "skipped_recent": 1,
  "errors": 0
}
```

### Pack Action Dispatch Integration

Add two new arms in `src/pack.rs` `execute_action()`:

```rust
#[cfg(not(target_arch = "wasm32"))]
"worktree-status" => {
    to_json_string(crate::worktree_tracker::worktree_status(&config))
}
#[cfg(target_arch = "wasm32")]
"worktree-status" => {
    Err(WitPluginError::internal("worktree-status not available in WASM"))
}
#[cfg(not(target_arch = "wasm32"))]
"recover-worktrees" => {
    let result = crate::worktree_tracker::recover_orphaned_worktrees(&config)?;
    to_json_string(
        serde_json::to_value(&result)
            .map_err(|e| WitPluginError::internal(format!("JSON error: {e}"))),
    )
}
#[cfg(target_arch = "wasm32")]
"recover-worktrees" => {
    Err(WitPluginError::internal("recover-worktrees not available in WASM"))
}
```

Update the `other =>` catch-all to list all available actions including `worktree-status`, `recover-worktrees`, and `cleanup-worktrees`.

### `use` of `cleanup_single_worktree` from Story 24.2

This story's `recover_orphaned_worktrees()` reuses `cleanup_single_worktree()` from Story 24.2 for the actual worktree+branch removal. Since both are in the same `worktree_tracker.rs` module, no cross-module import is needed — just call the function directly. If Story 24.2's `cleanup_single_worktree` was implemented as `fn` (not `pub fn`), it is still accessible within the module.

### `by_status` Counting in `worktree_status()`

Use a `BTreeMap<String, u32>` (not `HashMap`) for deterministic JSON key ordering:

```rust
let mut by_status = std::collections::BTreeMap::new();
for entry in &registry.entries {
    let status_str = serde_json::to_value(&entry.status)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| format!("{:?}", entry.status).to_lowercase());
    *by_status.entry(status_str).or_insert(0u32) += 1;
}
```

### Performance Requirement (NFR-AD-5)

The `worktree-status` query must return within 2 seconds for up to 20 worktrees. This is easily achievable since it only reads the JSON registry file (no git commands). The `git worktree list` command in `detect_untracked_worktrees()` is also fast. No special optimization is needed.

### Dependencies

This story depends on:
- Story 24.1: `WorktreeRegistry`, `WorktreeEntry`, `WorktreeStatus`, `load_registry()`, `save_registry()`, `now_iso8601()`, `is_leap_year()`
- Story 24.2: `cleanup_single_worktree()`, `CleanupOutcome`

All required production dependencies are already in `Cargo.toml`. **No new dependencies needed.**

### Anti-Patterns to Avoid

- **Do NOT** use `unwrap()` or `expect()` in production code — map all errors to `WitPluginError`
- **Do NOT** use `chrono` or `time` crate — use manual epoch calculation (see helpers above)
- **Do NOT** remove user-owned worktrees — only touch worktrees with `auto-dev/` branch prefix
- **Do NOT** use `HashMap` for JSON output — use `BTreeMap` for deterministic key ordering
- **Do NOT** panic if timestamp parsing fails — return `"unknown"` for age, treat as old for orphan detection
- **Do NOT** use `println!` or `eprintln!` — use `tracing` macros
- **Do NOT** force-delete branches (`-D`) — always use safe delete (`-d`)
- **Do NOT** skip `git worktree prune` — it must run first in the recovery pipeline to clean git's internal state
- **Do NOT** add retry loops around git commands — if git hangs, it is a system issue
- **Do NOT** use `git worktree list` without `--porcelain` — the default format is not machine-parseable

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests` in `worktree_tracker.rs`):
- Parse porcelain output from a hardcoded string (no real git repo needed for parsing tests)
- Test branch name extraction with various formats
- Test age formatting with known timestamps
- Test orphan age threshold detection
- Test status JSON structure
- Use `tempfile::tempdir()` for registry file tests

For tests that need to verify `is_old_enough_for_orphan()`, create entries with `created_at` set to a known past time (e.g., "2020-01-01T00:00:00Z" for definitely old, or use the current time minus 30 minutes for definitely recent).

**Integration tests** (optional, in `tests/` directory):
- Initialize a real git repo, create worktrees, register them, run recovery, verify cleanup
- Add `#[ignore]` attribute for CI environments that may not have git configured

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Epic 24, Story 24.3]
- [Source: src/worktree_tracker.rs — Story 24.1 and 24.2 functions used by this story]
- [Source: src/pack.rs — `execute_action()` dispatch, `to_json_string()` helper]
- [Source: src/auto_dev.rs — `auto_dev_status()` at line ~241 for status JSON pattern with `BTreeMap`]
- [Source: src/workspace.rs — `WorkspaceConfig` with `base_dir`]

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
- All 32 worktree-related tests pass (10 from 24-1, 7 from 24-2, 15 from 24-3)
- All 3 pack dispatch tests pass (cleanup, status, recover)
- All 202 lib tests pass with no regressions
- Clippy clean for all worktree_tracker and pack changes

### Completion Notes List
- Implemented run_git_worktree_prune() with non-fatal failure handling
- Implemented list_git_worktrees() parsing porcelain output via parse_porcelain_worktree_list()
- Implemented detect_untracked_worktrees() filtering only auto-dev/ branches
- Implemented recover_orphaned_worktrees() full 5-step pipeline: prune, detect, mark old failed, clean orphaned, save
- Implemented worktree_status() with BTreeMap for deterministic by_status ordering
- Implemented format_age() and parse_iso8601_to_epoch() without chrono dependency
- ORPHAN_AGE_THRESHOLD_SECS = 3600 (1 hour)
- Wired worktree-status and recover-worktrees actions in pack.rs with WASM gates
- Pack dispatch tests use isolated tempdir with git init for test isolation
- extract_ids_from_branch() parses auto-dev/{workflow_id}/{task_id} branch naming convention

### File List
- `src/worktree_tracker.rs` (modified) - added recovery, status, and age helper functions
- `src/pack.rs` (modified) - added worktree-status and recover-worktrees action dispatch
