# Story 22.2: Sync GitHub Issues to plugin-board as Tasks

Status: done

## Story

As an operator,
I want the auto-dev system to sync GitHub Issues into plugin-board as tasks,
So that issues become the task source for auto-dev without manual board task creation.

## Acceptance Criteria

1. **Given** `GitHubClient` can list issues, **When** `sync_issues_to_board()` is called, **Then** each open issue creates a board task with: title from issue title, description from issue body, metadata containing `issue_number`, `issue_url`, `labels`, `milestone`, and new tasks are created with status `ready-for-dev`.

2. **Given** an issue already has a corresponding board task (matched by `issue_number` in task metadata), **When** sync runs again, **Then** the existing task is updated with current issue title/body (not duplicated) and closed issues cause their board task to be moved to `done` status.

3. **Given** sync completes successfully, **When** the result is returned, **Then** it includes counts: `created`, `updated`, `skipped`, `closed`.

4. **Given** a new action `sync-github-issues` is added to `execute_action()` in `src/pack.rs`, **When** invoked via pack executor with `{"action": "sync-github-issues"}`, **Then** sync runs for the repo detected from the current workspace's git remote.

## Tasks / Subtasks

- [x] Task 1: Create `src/github_sync.rs` module with result types (AC: 1, 3)
  - [x] 1.1 Create new file `src/github_sync.rs`
  - [x] 1.2 Define `SyncResult` struct with `Serialize`: `created: u32`, `updated: u32`, `skipped: u32`, `closed: u32`
  - [x] 1.3 Define `SyncedTask` struct (internal) -- removed as unused, not needed for the sync logic
  - [x] 1.4 Add `#[cfg(not(target_arch = "wasm32"))] pub mod github_sync;` to `src/lib.rs` (after `github_client` declaration)

- [x] Task 2: Implement board task creation via Pulse API (AC: 1)
  - [x] 2.1 Add `create_task()` function to `src/board_client.rs` that POSTs to `http://127.0.0.1:{port}/api/v1/tasks` with JSON body `{ title, description, status, metadata }`, returns the new task ID
  - [x] 2.2 The POST body must include: `"title"`, `"description"`, `"status": "ready-for-dev"`, `"metadata": { "issue_number": N, "issue_url": "...", "labels": [...], "milestone": "..." }`
  - [x] 2.3 Parse the response to extract the created task's `id` field, return `Result<String, WitPluginError>`
  - [x] 2.4 Follow the existing `board_client.rs` error pattern: `api_err(format!("POST {url}: {e}"))`

- [x] Task 3: Implement board task lookup by issue_number (AC: 2)
  - [x] 3.1 Add `find_task_by_issue_number()` function to `src/board_client.rs` that lists all tasks and finds one whose metadata contains a matching `issue_number`
  - [x] 3.2 Fetch full task details including metadata via `GET /api/v1/tasks/{id}` for each candidate, or use the board data endpoint that returns metadata
  - [x] 3.3 Return `Result<Option<(String, String)>, WitPluginError>` where tuple is `(task_id, current_status)`

- [x] Task 4: Implement `sync_issues_to_board()` core logic (AC: 1, 2, 3)
  - [x] 4.1 Create `pub fn sync_issues_to_board(config: &WorkspaceConfig) -> Result<SyncResult, WitPluginError>`
  - [x] 4.2 Instantiate `GitHubClient::new()` and call `list_issues("open", None, None)` to get all open issues
  - [x] 4.3 Also call `list_issues("closed", None, None)` to detect recently closed issues for status sync
  - [x] 4.4 For each open issue: call `find_task_by_issue_number(issue.number)` to check for existing task
  - [x] 4.5 If no existing task: call `create_task()` with title, body, metadata, status `ready-for-dev` -> increment `created`
  - [x] 4.6 If existing task found: call `update_assignment(task_id, payload)` to update title/body -> increment `updated`
  - [x] 4.7 For each closed issue that has a board task not already in `done` status: update status to `done` -> increment `closed`
  - [x] 4.8 Issues with no changes needed: increment `skipped`
  - [x] 4.9 Return `SyncResult` with all counts
  - [x] 4.10 Add `tracing::info!` logs for each action: created, updated, closed, with issue number

- [x] Task 5: Wire `sync-github-issues` action into `execute_action()` (AC: 4)
  - [x] 5.1 In `src/pack.rs`, add match arm `"sync-github-issues"` that calls `crate::github_sync::sync_issues_to_board(&config)`
  - [x] 5.2 Serialize the `SyncResult` to JSON and return as `Ok(String)`
  - [x] 5.3 Update the `other =>` error message to include `sync-github-issues` in the available actions list

- [x] Task 6: Write unit tests (AC: 1, 2, 3)
  - [x] 6.1 `test_sync_result_serializes_correctly` -- verify JSON output shape
  - [x] 6.2 `test_issue_to_task_metadata_shape` -- verify metadata JSON contains issue_number, issue_url, labels, milestone
  - [x] 6.3 `test_sync_result_default_counts` -- verify initial counts are all zero

- [ ] Task 7: Write integration test (AC: 1, 2, 4) -- deferred to future; requires live Pulse API + plugin-board
  - [ ] 7.1 Create `tests/github_sync_integration.rs` with `#[ignore]` tests
  - [ ] 7.2 `test_sync_github_issues_creates_tasks` -- requires GITHUB_TOKEN + running Pulse API + plugin-board
  - [ ] 7.3 `test_sync_github_issues_is_idempotent` -- run sync twice, verify second run shows updated/skipped (not created)

## Dev Notes

### Dependency: Story 22.1 (`GitHubClient`)

This story depends on the `GitHubClient` struct created in Story 22.1 at `src/github_client.rs`. You will use:

```rust
use crate::github_client::{GitHubClient, GitHubIssue};

let client = GitHubClient::new()?;
let issues = client.list_issues(Some("open"), None, None)?;
```

`GitHubIssue` has these fields: `number: u64`, `title: String`, `body: Option<String>`, `labels: Vec<GitHubLabel>`, `milestone: Option<GitHubMilestone>`, `html_url: String`, `state: String`. `GitHubLabel` has `name: String`. `GitHubMilestone` has `title: String`, `number: u64`.

### Board Client Pattern (`src/board_client.rs`)

The existing `board_client.rs` uses **stateless functions** with `reqwest::blocking`. All functions follow this pattern:

```rust
fn board_api(path: &str) -> String {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    format!("http://127.0.0.1:{port}/api/v1/plugins/plugin-board/data/{path}")
}

fn api_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("Board API error: {msg}"))
}

// Existing functions you will reuse:
pub fn list_assignments(status_filter: Option<&str>) -> Result<Vec<Assignment>, WitPluginError>
pub fn update_assignment(task_id: &str, payload: &serde_json::Value) -> Result<(), WitPluginError>
```

The `Assignment` struct has: `id`, `title`, `status`, `description`, `priority`, `assignee`, `labels`, `workflow_id`. It does NOT currently have a `metadata` field. You may need to extend it or use a separate lookup.

**New function needed**: `create_task()` must POST to the Pulse Task API (not the board plugin data endpoint). The board plugin reads from the Pulse task store, so creating a task via `POST /api/v1/tasks` makes it visible on the board. Use the same port/URL pattern as `update_assignment()` which already uses `http://127.0.0.1:{port}/api/v1/tasks/{id}/metadata`.

### Task Creation API

The Pulse Task API supports creating tasks via POST:

```
POST http://127.0.0.1:{port}/api/v1/tasks
Content-Type: application/json

{
  "title": "Issue title here",
  "description": "Issue body here",
  "status": "ready-for-dev",
  "metadata": {
    "issue_number": 42,
    "issue_url": "https://github.com/owner/repo/issues/42",
    "labels": ["bug", "auto-dev"],
    "milestone": "Sprint 5",
    "source": "github-sync"
  }
}
```

The response returns the created task with its `id`.

### Finding Tasks by Issue Number (Duplicate Detection)

To avoid duplicating tasks, you need to check if a board task already exists for a given GitHub issue number. The approach:

1. Call `list_assignments(None)` to get all tasks
2. For each task, fetch its metadata via `GET /api/v1/tasks/{id}`
3. Check if `metadata.issue_number` matches

This is O(n) per issue. For performance at scale, consider building a local HashMap of `issue_number -> task_id` by scanning all tasks once at the start of sync, then doing lookups against that map.

### Module Structure

- **New file**: `src/github_sync.rs` -- contains `sync_issues_to_board()` and supporting types
- **Modified file**: `src/board_client.rs` -- add `create_task()` and `find_task_by_issue_number()`
- **Modified file**: `src/pack.rs` -- add `"sync-github-issues"` match arm in `execute_action()`
- **Modified file**: `src/lib.rs` -- add `pub mod github_sync;` behind WASM gate
- The module follows the flat `src/*.rs` convention -- no nested directories

### Adding the Module to `lib.rs`

In `src/lib.rs`, add the new module declaration behind the WASM gate, grouped with the other HTTP-dependent modules:

```rust
#[cfg(not(target_arch = "wasm32"))]
pub mod github_client;   // <-- from Story 22.1
#[cfg(not(target_arch = "wasm32"))]
pub mod github_sync;     // <-- NEW for this story
```

### Adding the Action to `pack.rs`

In `src/pack.rs`, the `execute_action()` function dispatches on `input.action.as_str()`. Add the new arm before the `other =>` catch-all:

```rust
"sync-github-issues" => {
    let result = crate::github_sync::sync_issues_to_board(&config)?;
    to_json_string(
        serde_json::to_value(&result)
            .map_err(|e| WitPluginError::internal(format!("JSON error: {e}"))),
    )
}
```

Also update the error message in the `other =>` arm to include `sync-github-issues` in the available actions list.

### Metadata JSON Shape

The metadata stored on each synced task must include these fields so Stories 22.3 and 22.4 can use them:

```json
{
  "issue_number": 42,
  "issue_url": "https://github.com/owner/repo/issues/42",
  "labels": ["bug", "auto-dev"],
  "milestone": "Sprint 5",
  "source": "github-sync"
}
```

The `source: "github-sync"` field allows distinguishing synced tasks from manually created ones.

### Error Handling Constraints

- All errors map to `WitPluginError` using `internal()` for API/network errors and `invalid_input()` for bad configuration
- NEVER `unwrap()` or `expect()` in production code
- NEVER log GITHUB_TOKEN values -- the `GitHubClient` from 22.1 handles token management
- Use `tracing::info!` for sync actions, `tracing::warn!` for skipped/failed items
- If `GitHubClient::new()` fails (e.g., no token), propagate the error -- do not silently skip

### Anti-Patterns to Avoid

- **Do NOT** use async reqwest -- use `reqwest::blocking` only
- **Do NOT** `unwrap()` or `expect()` in production code
- **Do NOT** create duplicate tasks -- always check for existing task by issue_number first
- **Do NOT** log GITHUB_TOKEN or any credential values
- **Do NOT** use `println!` or `eprintln!` -- use `tracing` macros only
- **Do NOT** use `HashMap` for serialized JSON output -- use `serde_json::json!()` or typed structs with `Serialize`
- **Do NOT** fetch closed issues if it will exceed API rate limits -- consider limiting to recent closes

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests` in `github_sync.rs`):
- Test `SyncResult` serialization
- Test metadata JSON shape construction
- Test the mapping from `GitHubIssue` fields to task creation payload

**Integration tests** (`tests/github_sync_integration.rs` with `#[ignore]`):
- Requires: `GITHUB_TOKEN` env var, running Pulse API, running plugin-board
- Test full sync flow: create issues, run sync, verify tasks appear on board
- Test idempotency: run sync twice, verify no duplicates

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Story 22.2]
- [Source: src/board_client.rs -- HTTP client pattern, update_assignment(), list_assignments()]
- [Source: src/github_client.rs -- GitHubClient, GitHubIssue types (from Story 22.1)]
- [Source: src/pack.rs -- execute_action() dispatch pattern]
- [Source: src/lib.rs -- module declarations with WASM gate]
- [Source: src/auto_dev.rs -- board integration pattern, pick_next_task()]
- [Source: src/pulse_api.rs -- Pulse Task API client pattern]

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
N/A

### Completion Notes List
- Created `src/github_sync.rs` with `SyncResult`, `sync_issues_to_board()`, open/closed issue sync logic
- Added `create_task()`, `find_task_by_issue_number()`, `get_task_metadata()` to `src/board_client.rs`
- Wired `sync-github-issues` action in `src/pack.rs`
- Added `pub mod github_sync` to `src/lib.rs` behind WASM gate
- 4 unit tests for SyncResult serialization, metadata shape, default counts, no-milestone case
- All 163 tests pass, clippy clean
- Removed unused `SyncedTask` struct to satisfy clippy dead_code warning
- Integration tests (Task 7) deferred -- require live Pulse API + plugin-board

### File List
- `src/github_sync.rs` (new) - GitHub issue sync module
- `src/board_client.rs` (modified) - Added create_task, find_task_by_issue_number, get_task_metadata
- `src/pack.rs` (modified) - Added sync-github-issues action
- `src/lib.rs` (modified) - Added pub mod github_sync behind WASM gate

### Change Log
- 2026-03-27: Story 22-2 implemented and moved to review
