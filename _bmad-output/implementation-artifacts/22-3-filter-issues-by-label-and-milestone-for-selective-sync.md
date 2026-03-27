# Story 22.3: Filter Issues by Label and Milestone for Selective Sync

Status: review

## Story

As an operator,
I want to filter which GitHub Issues are synced by label and/or milestone,
So that only issues marked for auto-dev processing become tasks.

## Acceptance Criteria

1. **Given** workspace config has `github_sync: { filter_labels: ["auto-dev"] }`, **When** sync runs, **Then** only issues with the `auto-dev` label are synced to the board.

2. **Given** workspace config has `github_sync: { filter_milestone: "Sprint 5" }`, **When** sync runs, **Then** only issues in that milestone are synced.

3. **Given** both label and milestone filters are set, **When** sync runs, **Then** issues must match ALL filters (AND logic).

4. **Given** no `github_sync` section exists in config, **When** sync runs, **Then** all open issues are synced (backward-compatible default via `#[serde(default)]`).

5. **Given** a new `GitHubSyncConfig` struct is added to `src/workspace.rs`, **When** `WorkspaceConfig` is deserialized, **Then** the optional `github_sync` field parses correctly with sensible defaults.

## Tasks / Subtasks

- [x] Task 1: Add `GitHubSyncConfig` struct to `src/workspace.rs` (AC: 5)
  - [x] 1.1 Define `GitHubSyncConfig` struct with `#[derive(Debug, Clone, Default, Deserialize)]`
  - [x] 1.2 Add field `filter_labels: Vec<String>` with `#[serde(default)]` -- empty means no label filter
  - [x] 1.3 Add field `filter_milestone: Option<String>` with `#[serde(default)]` -- None means no milestone filter
  - [x] 1.4 Add `pub github_sync: GitHubSyncConfig` field to `WorkspaceConfig` struct
  - [x] 1.5 Add `github_sync: Option<GitHubSyncConfig>` field to the private `ConfigYaml` struct
  - [x] 1.6 Wire `github_sync` into `WorkspaceConfig::from_base_dir()`: `github_sync: yaml.github_sync.unwrap_or_default()`
  - [x] 1.7 Wire `github_sync` into `WorkspaceConfig::default_for()`: `github_sync: GitHubSyncConfig::default()`

- [x] Task 2: Apply label filter in sync logic (AC: 1, 3)
  - [x] 2.1 In `src/github_sync.rs`, modify `sync_issues_to_board()` to read `GitHubSyncConfig` from the `WorkspaceConfig`
  - [x] 2.2 Client-side filtering: skip issues that don't have ALL required labels (AND logic)
  - [x] 2.3 Applied as a client-side safety net via `matches_issue()` function

- [x] Task 3: Apply milestone filter in sync logic (AC: 2, 3)
  - [x] 3.1 Client-side milestone filtering by comparing `issue.milestone.title` against configured name
  - [x] 3.2 Skipped server-side milestone filtering (API takes number, not title)
  - [x] 3.3 Client-side approach used as recommended for simplicity

- [x] Task 4: Ensure backward compatibility with no config (AC: 4)
  - [x] 4.1 Empty filter_labels + None filter_milestone = all issues pass through
  - [x] 4.2 Verified existing config.yaml without github_sync still parses
  - [x] 4.3 #[serde(default)] on both struct and ConfigYaml field

- [x] Task 5: Add helper function for `GitHubSyncConfig` (AC: 1, 2, 3, 4)
  - [x] 5.1 Added `pub fn matches_issue(config: &GitHubSyncConfig, issue: &GitHubIssue) -> bool` in github_sync.rs
  - [x] 5.2 Label check: ALL configured labels required (AND logic)
  - [x] 5.3 Milestone check: title must match
  - [x] 5.4 Both use AND logic
  - [x] 5.5 Empty/None filters return true

- [x] Task 6: Update sync loop to use filter (AC: 1, 2, 3)
  - [x] 6.1 In `sync_issues_to_board()`, calls `matches_issue()` before processing each issue
  - [x] 6.2 Non-matching issues increment `skipped`
  - [x] 6.3 Added `tracing::debug!` for skipped issues

- [x] Task 7: Write unit tests for config parsing (AC: 4, 5)
  - [x] 7.1 `test_github_sync_config_default`
  - [x] 7.2 `test_github_sync_config_parsed`
  - [x] 7.3 `test_config_yaml_backward_compatible_without_github_sync`
  - [x] 7.4 `test_config_yaml_with_github_sync`

- [x] Task 8: Write unit tests for filter logic (AC: 1, 2, 3, 4)
  - [x] 8.1 `test_matches_issue_no_filters`
  - [x] 8.2 `test_matches_issue_label_filter_matches`
  - [x] 8.3 `test_matches_issue_label_filter_no_match`
  - [x] 8.4 `test_matches_issue_milestone_filter_matches`
  - [x] 8.5 `test_matches_issue_milestone_filter_no_match`
  - [x] 8.6 `test_matches_issue_both_filters_and_logic`
  - [x] 8.7 `test_matches_issue_multiple_labels_all_required`

## Dev Notes

### Existing `WorkspaceConfig` Structure (`src/workspace.rs`)

The `WorkspaceConfig` struct is the central configuration object. It already has optional sub-configs with serde defaults. Follow the exact same pattern used by `AutoDevConfig`:

```rust
// Existing pattern in workspace.rs:
#[derive(Debug, Clone, Deserialize)]
pub struct AutoDevConfig {
    #[serde(default = "default_auto_dev_retries")]
    pub max_retries: u32,
    #[serde(default = "default_auto_dev_max_tasks")]
    pub max_tasks: u32,
    #[serde(default)]
    pub skip_validation: bool,
}

impl Default for AutoDevConfig {
    fn default() -> Self {
        Self {
            max_retries: default_auto_dev_retries(),
            max_tasks: default_auto_dev_max_tasks(),
            skip_validation: false,
        }
    }
}
```

Your `GitHubSyncConfig` should follow this same pattern but is simpler since all defaults are empty/None:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GitHubSyncConfig {
    #[serde(default)]
    pub filter_labels: Vec<String>,
    #[serde(default)]
    pub filter_milestone: Option<String>,
}
```

### Where to Add Fields in `WorkspaceConfig`

The `WorkspaceConfig` struct (line 7 of `src/workspace.rs`) has these fields:

```rust
pub struct WorkspaceConfig {
    pub base_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub workflows_dir: PathBuf,
    pub workflows: WorkflowFilter,
    pub defaults: DefaultSettings,
    pub use_injection_pipeline: bool,
    pub auto_dev: AutoDevConfig,
    // ADD: pub github_sync: GitHubSyncConfig,
}
```

You must also update:
1. The private `ConfigYaml` struct (line 94) -- add `github_sync: Option<GitHubSyncConfig>`
2. `WorkspaceConfig::from_base_dir()` (line 123) -- add `github_sync: yaml.github_sync.unwrap_or_default()`
3. `WorkspaceConfig::default_for()` (line 168) -- add `github_sync: GitHubSyncConfig::default()`

### Config YAML Format

After this story, `config/config.yaml` can optionally include:

```yaml
# Existing fields...
plugin_dir: "config/plugins"
auto_dev:
  max_retries: 2

# NEW: GitHub sync filtering
github_sync:
  filter_labels:
    - auto-dev
    - ready
  filter_milestone: "Sprint 5"
```

When `github_sync` is absent, all issues are synced (backward compatible).

### Filter Logic: `matches_issue()` Method

Add this method on `GitHubSyncConfig`. It needs access to the `GitHubIssue` type from `src/github_client.rs`:

```rust
impl GitHubSyncConfig {
    pub fn matches_issue(&self, issue: &crate::github_client::GitHubIssue) -> bool {
        // Label filter: issue must have ALL configured labels
        if !self.filter_labels.is_empty() {
            let issue_labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
            for required in &self.filter_labels {
                if !issue_labels.contains(&required.as_str()) {
                    return false;
                }
            }
        }

        // Milestone filter: issue must be in the configured milestone
        if let Some(ref required_milestone) = self.filter_milestone {
            match &issue.milestone {
                Some(m) if m.title == *required_milestone => {}
                _ => return false,
            }
        }

        true
    }
}
```

**Important**: This method should live in `src/github_sync.rs` (not `src/workspace.rs`) to avoid adding a dependency from `workspace.rs` to `github_client.rs`. Alternatively, implement it as a standalone function `fn matches_issue(config: &GitHubSyncConfig, issue: &GitHubIssue) -> bool` in `github_sync.rs`.

### WASM Gate Consideration

`src/workspace.rs` is NOT behind a WASM gate -- it compiles for all targets. The `GitHubSyncConfig` struct only uses `String`, `Vec<String>`, and `Option<String>` which are all WASM-safe. The `matches_issue()` method references `GitHubIssue` which is in `github_client.rs` (WASM-gated). Therefore:

- `GitHubSyncConfig` struct definition: safe to put in `workspace.rs` (no WASM issue)
- `matches_issue()` method: must go in `github_sync.rs` (which is WASM-gated) because it references WASM-gated types

### Integration with `sync_issues_to_board()` from Story 22.2

In `src/github_sync.rs`, the sync function from Story 22.2 must be updated to use the filter:

```rust
pub fn sync_issues_to_board(config: &WorkspaceConfig) -> Result<SyncResult, WitPluginError> {
    let client = GitHubClient::new()?;
    let issues = client.list_issues(Some("open"), None, None)?;

    let mut result = SyncResult::default();

    for issue in issues {
        // NEW: Apply filter from config
        if !matches_issue(&config.github_sync, &issue) {
            tracing::debug!(
                plugin = "coding-pack",
                issue_number = issue.number,
                "Skipping issue: does not match sync filter"
            );
            result.skipped += 1;
            continue;
        }
        // ... existing sync logic ...
    }
    Ok(result)
}
```

### GitHub API Server-Side vs Client-Side Filtering

The GitHub Issues API supports `labels` and `milestone` query parameters:
- `?labels=auto-dev,bug` -- matches issues with ANY of the listed labels (OR logic)
- `?milestone=5` -- filters by milestone NUMBER (not title)

Since the AC requires AND logic for labels (issue must have ALL configured labels), and the API uses OR logic, **client-side filtering is required**. You can optionally pass labels to the API for a rough pre-filter, but must still validate client-side.

For milestones, the API takes a number but our config uses a title string. Use client-side filtering by comparing `issue.milestone.title`.

### Error Handling

- Config parsing errors should never crash -- `#[serde(default)]` handles missing fields
- Filter logic is pure and cannot fail -- it returns bool
- If `GitHubClient::new()` fails due to missing token, the error propagates from `sync_issues_to_board()`

### Anti-Patterns to Avoid

- **Do NOT** put `matches_issue()` in `workspace.rs` -- it would create a dependency on WASM-gated `github_client.rs`
- **Do NOT** require label OR logic -- the AC says AND: issue must have ALL configured labels
- **Do NOT** break backward compatibility -- configs without `github_sync` must still work
- **Do NOT** use `unwrap()` or `expect()` in production code
- **Do NOT** fetch milestone number from GitHub API just for filtering -- use client-side title comparison
- **Do NOT** modify `GitHubClient::list_issues()` signature if it breaks Story 22.1 -- add filtering on top

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests`):

In `src/workspace.rs`:
- Config parsing with and without `github_sync` section
- Default values when section is absent
- Full section parsing with labels and milestone

In `src/github_sync.rs`:
- `matches_issue()` with various combinations of filters and issue data
- Build test `GitHubIssue` values with helper function for concise tests:
```rust
fn test_issue(number: u64, labels: &[&str], milestone: Option<&str>) -> GitHubIssue {
    GitHubIssue {
        number,
        title: format!("Test issue {number}"),
        body: None,
        labels: labels.iter().map(|l| GitHubLabel { name: l.to_string() }).collect(),
        milestone: milestone.map(|t| GitHubMilestone { title: t.to_string(), number: 1 }),
        html_url: format!("https://github.com/test/repo/issues/{number}"),
        state: "open".to_string(),
    }
}
```

### Files to Modify

- `src/workspace.rs` -- add `GitHubSyncConfig` struct, add field to `WorkspaceConfig`, `ConfigYaml`, `from_base_dir()`, `default_for()`
- `src/github_sync.rs` -- add `matches_issue()` function, integrate filter into sync loop
- No changes to `src/lib.rs` or `src/pack.rs` (those were done in Story 22.2)

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Story 22.3]
- [Source: src/workspace.rs -- WorkspaceConfig, ConfigYaml, AutoDevConfig pattern]
- [Source: src/github_sync.rs -- sync_issues_to_board() from Story 22.2]
- [Source: src/github_client.rs -- GitHubIssue, GitHubLabel, GitHubMilestone types from Story 22.1]

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
N/A

### Completion Notes List
- Added `GitHubSyncConfig` struct to `workspace.rs` with `filter_labels` and `filter_milestone`
- Wired into `WorkspaceConfig`, `ConfigYaml`, `from_base_dir()`, `default_for()`
- Added `matches_issue()` function in `github_sync.rs` (not workspace.rs to avoid WASM gate issue)
- Integrated filter into `sync_issues_to_board()` loop with debug logging for skipped issues
- 4 workspace config tests + 7 filter logic tests = 11 new tests, all passing
- Clippy clean, backward compatible with existing configs

### File List
- `src/workspace.rs` (modified) - Added GitHubSyncConfig struct, wired into WorkspaceConfig
- `src/github_sync.rs` (modified) - Added matches_issue() function, integrated filter into sync loop

### Change Log
- 2026-03-27: Story 22-3 implemented and moved to review
