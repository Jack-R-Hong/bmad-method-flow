# Story 24.2: Auto-Cleanup Worktrees on Completion and Merge

Status: done

## Story

As a platform operator,
I want completed and merged worktrees to be automatically cleaned up,
So that disk space is reclaimed and stale branches don't accumulate.

## Acceptance Criteria

1. **Given** a worktree entry has status `completed`, **When** `cleanup_completed_worktrees()` is called, **Then** the worktree is removed via `git worktree remove <path>`, the tracking branch is deleted via `git branch -d <branch>` (safe delete, not force), and the registry entry is removed from the registry file.

2. **Given** cleanup runs on a worktree, **When** `git worktree remove` succeeds but `git branch -d` fails (e.g., unmerged branch), **Then** the worktree is still marked as cleaned, the branch deletion failure is logged as a warning, and the entry is still removed from the registry.

3. **Given** a worktree entry has status `active`, **When** `cleanup_completed_worktrees()` runs, **Then** the active worktree is skipped — never remove active worktrees.

4. **Given** cleanup processes multiple worktrees, **When** one worktree removal fails (e.g., path already deleted), **Then** the remaining worktrees are still processed and the result includes accurate counts.

5. **Given** a new action `cleanup-worktrees` is added to `execute_action()` in `src/pack.rs`, **When** invoked with `{"action": "cleanup-worktrees"}`, **Then** all `completed` worktrees are cleaned up and the result includes counts: `removed`, `skipped`, `errors`.

6. **Given** cleanup operates on a worktree, **When** timing is measured, **Then** cleanup completes within 10 seconds per worktree (NFR-AD-4).

7. **Given** `cargo clippy -- -D warnings` is run, **When** the new functions and pack action compile, **Then** no warnings or errors are reported.

## Tasks / Subtasks

- [x] Task 1: Implement `cleanup_worktree()` single-entry cleanup in `src/worktree_tracker.rs` (AC: 1, 2)
  - [x] 1.1 Implement `fn remove_git_worktree(config: &WorkspaceConfig, worktree_path: &str) -> Result<(), WitPluginError>` — runs `git worktree remove <path>` via `std::process::Command` with `current_dir` set to `config.base_dir`. If the command fails because the path does not exist, treat as success (already cleaned). Map other failures to `WitPluginError::internal()`.
  - [x] 1.2 Implement `fn delete_git_branch(config: &WorkspaceConfig, branch_name: &str) -> Result<(), WitPluginError>` — runs `git branch -d <branch>` via `std::process::Command` with `current_dir` set to `config.base_dir`. Use `-d` (safe delete, not `-D` force delete). If the branch does not exist or is unmerged, log a warning and return `Ok(())` — branch deletion failures are non-fatal.
  - [x] 1.3 Implement `fn cleanup_single_worktree(config: &WorkspaceConfig, entry: &WorktreeEntry) -> CleanupOutcome` — calls `remove_git_worktree()`, then `delete_git_branch()`. Returns a `CleanupOutcome` enum: `Removed`, `AlreadyGone`, `Error(String)`.

- [x] Task 2: Implement `cleanup_completed_worktrees()` (AC: 1, 3, 4, 6)
  - [x] 2.1 Define `CleanupOutcome` enum: `Removed`, `AlreadyGone`, `Error(String)`. Derive `Debug`.
  - [x] 2.2 Define `CleanupResult` struct: `removed: u32`, `skipped: u32`, `errors: u32`, `details: Vec<CleanupDetail>`. Derive `Debug, Serialize`.
  - [x] 2.3 Define `CleanupDetail` struct: `worktree_path: String`, `outcome: String`. Derive `Debug, Serialize`.
  - [x] 2.4 Implement `pub fn cleanup_completed_worktrees(config: &WorkspaceConfig) -> Result<CleanupResult, WitPluginError>`.
  - [x] 2.5 Load registry. Iterate entries. Skip any entry with `status != Completed` (increment `skipped` counter for non-completed entries that are `Active`; do NOT skip `Failed` — only skip `Active`). Note: this function cleans `Completed` entries only. `Failed` and `Orphaned` entries are handled by Story 24.3.
  - [x] 2.6 For each `Completed` entry, call `cleanup_single_worktree()`. Track outcome in `CleanupDetail`.
  - [x] 2.7 After processing, rebuild the registry entries list: keep only entries that were NOT successfully removed. Save the updated registry atomically.
  - [x] 2.8 Return `CleanupResult` with accurate counts.
  - [x] 2.9 Add tracing logs: `tracing::info!(plugin = "coding-pack", removed = result.removed, skipped = result.skipped, errors = result.errors, "worktree cleanup complete");`

- [x] Task 3: Add `cleanup-worktrees` action to pack dispatch (AC: 5)
  - [x] 3.1 In `src/pack.rs`, add a new match arm in `execute_action()` for `"cleanup-worktrees"`.
  - [x] 3.2 Call `crate::worktree_tracker::cleanup_completed_worktrees(&config)`.
  - [x] 3.3 Serialize the `CleanupResult` to JSON via `serde_json::to_value()` and return through `to_json_string()`.
  - [x] 3.4 Update the error message in the `other =>` catch-all arm to include `"cleanup-worktrees"` in the list of available actions.
  - [x] 3.5 Gate the action handler with `#[cfg(not(target_arch = "wasm32"))]` — for WASM builds, return `WitPluginError::internal("cleanup-worktrees not available in WASM")`.

- [x] Task 4: Write unit tests (AC: 1, 2, 3, 4)
  - [x] 4.1 `test_cleanup_completed_removes_entry` — register a worktree with status `Completed`, run `cleanup_completed_worktrees()`, verify registry is empty afterward. Use a temp dir so `git worktree remove` on a nonexistent path is handled gracefully.
  - [x] 4.2 `test_cleanup_skips_active` — register an `Active` worktree and a `Completed` worktree, run cleanup, verify only the completed one is removed and the active one remains in the registry.
  - [x] 4.3 `test_cleanup_result_counts` — register multiple worktrees with mixed statuses, run cleanup, verify `removed`, `skipped`, `errors` counts.
  - [x] 4.4 `test_cleanup_nonexistent_path_still_removes_entry` — register a worktree with a path that does not exist on disk, run cleanup, verify the entry is removed from registry (already-gone counts as success).
  - [x] 4.5 `test_remove_git_worktree_command` — verify `remove_git_worktree()` constructs the correct `git worktree remove` command (use a real temp git repo or check the error gracefully).
  - [x] 4.6 `test_delete_git_branch_safe_delete` — verify `delete_git_branch()` uses `-d` (not `-D`).
  - [x] 4.7 `test_cleanup_worktrees_action_dispatch` — call `execute_action()` with `{"action": "cleanup-worktrees"}`, verify it returns valid JSON with `removed`, `skipped`, `errors` fields.
  - [x] 4.8 Use `tempfile::tempdir()` for all tests that write to disk.

## Dev Notes

### Git Worktree Commands Reference

The cleanup operations use standard git commands via `std::process::Command`:

```rust
// Remove a worktree — removes the directory and git's worktree metadata
std::process::Command::new("git")
    .args(["worktree", "remove", worktree_path])
    .current_dir(&config.base_dir)
    .output()

// Safe-delete a branch (fails if unmerged — that's intentional)
std::process::Command::new("git")
    .args(["branch", "-d", branch_name])
    .current_dir(&config.base_dir)
    .output()
```

**Important**: Always set `.current_dir(&config.base_dir)` so git finds the correct repository. Without this, git may operate on the wrong repo or fail to find one.

**Important**: Use `git branch -d` (safe delete), NEVER `git branch -D` (force delete). Safe delete refuses to delete unmerged branches, which prevents data loss. If the branch has unmerged changes, the developer should handle it manually.

### Handling Already-Deleted Worktrees

A worktree path may no longer exist on disk if:
- The disk directory was manually deleted
- A previous cleanup attempt partially succeeded
- The OS cleaned up temp storage

When `git worktree remove` fails because the path is already gone, check the error output for "is not a working tree" or the exit code. Treat this as a success — the goal (remove the worktree) is already achieved.

```rust
fn remove_git_worktree(config: &WorkspaceConfig, worktree_path: &str) -> Result<(), WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["worktree", "remove", worktree_path])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git worktree remove: {e}")))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    // If the worktree path doesn't exist, treat as already cleaned
    if stderr.contains("is not a working tree") || stderr.contains("No such file or directory") {
        tracing::debug!(plugin = "coding-pack", path = worktree_path, "worktree already removed");
        return Ok(());
    }

    Err(WitPluginError::internal(format!(
        "git worktree remove failed for '{}': {}",
        worktree_path,
        stderr.trim()
    )))
}
```

### Branch Deletion Is Non-Fatal

Branch deletion with `-d` may fail for legitimate reasons:
- Branch has unmerged commits (git refuses safe delete)
- Branch was already deleted
- Branch is currently checked out somewhere

In all cases, log a warning but return `Ok(())`:

```rust
fn delete_git_branch(config: &WorkspaceConfig, branch_name: &str) -> Result<(), WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["branch", "-d", branch_name])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git branch -d: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            plugin = "coding-pack",
            branch = branch_name,
            error = %stderr.trim(),
            "branch deletion failed — non-fatal"
        );
    }
    Ok(())
}
```

### Pack Action Dispatch Integration

Add the action in `src/pack.rs` in the `execute_action()` match block. Place it after the existing `auto-dev-watch` arm and before the `other =>` catch-all:

```rust
#[cfg(not(target_arch = "wasm32"))]
"cleanup-worktrees" => {
    let result = crate::worktree_tracker::cleanup_completed_worktrees(&config)?;
    to_json_string(
        serde_json::to_value(&result)
            .map_err(|e| WitPluginError::internal(format!("JSON error: {e}"))),
    )
}
#[cfg(target_arch = "wasm32")]
"cleanup-worktrees" => {
    Err(WitPluginError::internal("cleanup-worktrees not available in WASM"))
}
```

Also update the error message string in the `other =>` arm to include `cleanup-worktrees`:

```rust
other => Err(WitPluginError::not_found(format!(
    "Unknown action: '{}'. Available: validate-pack, validate-workflows, list-workflows, list-plugins, status, execute-workflow, data-query, data-mutate, auto-dev-status, auto-dev-next, auto-dev-watch, cleanup-worktrees",
    other
))),
```

### Expected JSON Output Format

The `cleanup-worktrees` action returns:

```json
{
  "removed": 3,
  "skipped": 1,
  "errors": 0,
  "details": [
    {"worktree_path": "/path/to/.worktrees/auto-dev/coding-story-dev/task-42", "outcome": "removed"},
    {"worktree_path": "/path/to/.worktrees/auto-dev/coding-bug-fix/task-99", "outcome": "removed"},
    {"worktree_path": "/path/to/.worktrees/auto-dev/coding-quick-dev/task-7", "outcome": "already_gone"},
    {"worktree_path": "/path/to/.worktrees/auto-dev/coding-feature-dev/task-1", "outcome": "skipped_active"}
  ]
}
```

### Which Statuses Get Cleaned

- `Active` — NEVER cleaned. Always skipped. Counted in `skipped`.
- `Completed` — cleaned by `cleanup_completed_worktrees()`.
- `Failed` — NOT cleaned by this function. Handled by `recover_orphaned_worktrees()` in Story 24.3.
- `Orphaned` — NOT cleaned by this function. Handled by `recover_orphaned_worktrees()` in Story 24.3.

This separation ensures that failed worktrees are given a grace period (1 hour, enforced in 24.3) before cleanup, and that cleanup of completed vs. orphaned worktrees can be triggered independently.

### Performance Requirement (NFR-AD-4)

Each worktree cleanup must complete within 10 seconds. The git commands (`worktree remove` + `branch -d`) typically complete in under 1 second each. The 10-second budget is generous but serves as a safeguard. Do NOT add an explicit timeout — if git hangs, it's a system issue, not a plugin issue. The NFR is a design constraint, not an enforcement requirement.

### Dependencies

This story depends on Story 24.1 (worktree_tracker module with `WorktreeRegistry`, `WorktreeEntry`, `WorktreeStatus`, `load_registry()`, `save_registry()`).

All required production dependencies are already in `Cargo.toml`:
- `serde`, `serde_json` — serialization
- `tracing` — logging
- `pulse-plugin-sdk` — error types

**No new dependencies needed.**

### Anti-Patterns to Avoid

- **Do NOT** use `git branch -D` (force delete) — always use `-d` (safe delete) to prevent data loss
- **Do NOT** remove active worktrees under any circumstances — an active worktree may have uncommitted work
- **Do NOT** abort cleanup if one worktree fails — process all eligible worktrees and report aggregate results
- **Do NOT** use `unwrap()` or `expect()` in production code
- **Do NOT** use `println!` or `eprintln!` — use `tracing` macros
- **Do NOT** use `std::process::Command::new("bash")` for git commands — call `git` directly
- **Do NOT** add `--force` to `git worktree remove` — let it fail if there are uncommitted changes (the developer should handle that)
- **Do NOT** remove the registry file entirely after cleanup — save it with remaining entries (may still have active entries)
- **Do NOT** use `rm -rf` to remove worktree directories — always use `git worktree remove` so git's internal tracking is updated

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests` in `worktree_tracker.rs`):
- Most tests can work with a temp dir and a manually populated registry JSON file
- Tests for `remove_git_worktree` and `delete_git_branch` will get errors on systems without git repos — that's fine; test that errors are handled gracefully (the function should not panic)
- Test the pack action dispatch via `execute_action()` in `pack.rs` tests

**Integration tests** (optional, in `tests/` directory):
- Set up a real git repo with worktrees, register them, run cleanup, verify filesystem state
- These are more fragile and can be added later

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Epic 24, Story 24.2]
- [Source: src/worktree_tracker.rs — Story 24.1 module with `WorktreeRegistry`, `load_registry()`, `save_registry()`]
- [Source: src/pack.rs — `execute_action()` dispatch at line ~42-103, `to_json_string()` helper]
- [Source: src/auto_dev.rs — `std::process::Command` usage pattern in `run_validation()` at line ~88-92]
- [Source: src/workspace.rs — `WorkspaceConfig` with `base_dir`]

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
- All 17 worktree tests pass (10 from 24-1 + 7 new cleanup tests)
- Pack dispatch test passes (cleanup_worktrees_action_dispatch)
- All 181 lib tests pass with no regressions

### Completion Notes List
- Implemented remove_git_worktree() with localization-safe fallback (checks filesystem existence when git error messages don't match known patterns)
- Implemented delete_git_branch() using -d (safe delete, never -D), returns Ok on failure (non-fatal)
- Implemented cleanup_single_worktree() composing remove + branch delete
- Implemented cleanup_completed_worktrees() that only cleans Completed entries, skips Active, preserves Failed/Orphaned
- Defined CleanupOutcome, CleanupResult, CleanupDetail types
- Wired cleanup-worktrees action in pack.rs with WASM gate
- Tests use init_git_repo() helper to ensure git commands work in temp dirs

### File List
- `src/worktree_tracker.rs` (modified) - added cleanup types and functions
- `src/pack.rs` (modified) - added cleanup-worktrees action dispatch
