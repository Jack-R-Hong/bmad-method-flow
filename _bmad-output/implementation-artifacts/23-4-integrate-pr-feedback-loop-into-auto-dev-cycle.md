# Story 23.4: Integrate PR Feedback Loop into Auto-Dev Cycle

Status: ready-for-dev

## Story

As an operator,
I want the auto-dev system to automatically detect PRs needing fixes and trigger the `coding-pr-fix` workflow,
So that the review-fix cycle runs autonomously without manual intervention.

## Acceptance Criteria

1. **Given** `auto_dev.rs` has the existing `pick_next_task()` flow, **When** `check_pending_reviews()` is added as a secondary task source, **Then** PRs with `changes_requested` status are returned as fix tasks with priority higher than regular board tasks. Specifically, `pick_next_task()` first checks `check_pending_reviews()` and returns a PR fix task if one exists; only if no PR fixes are pending does it fall through to the original board `list_assignments()` path.

2. **Given** a PR fix task is picked, **When** the auto-dev loop executes it, **Then** the `coding-pr-fix` workflow is selected automatically (not via label routing -- the fix task has `workflow_id: "coding-pr-fix"` set explicitly), **And** template variables include `pr_number`, `pr_branch`, `pr_url` from the PR metadata, **And** the board task status is updated to `in-progress` during fix, then back to `review` after push.

3. **Given** the fix workflow completes and pushes commits, **When** the re-request review step runs, **Then** a GitHub API call requests re-review from the original reviewers via `POST /repos/{owner}/{repo}/pulls/{pr_number}/requested_reviewers` with body `{"reviewers": ["reviewer1", "reviewer2"]}`.

4. **Given** a PR has been approved (no changes requested), **When** `check_pending_reviews()` scans it, **Then** the PR is skipped (not queued for fixes).

5. **Given** a PR has review state `pending` (no reviews yet), **When** `check_pending_reviews()` scans it, **Then** the PR is skipped (only `changes_requested` triggers a fix task).

6. **Given** `GITHUB_TOKEN` is not set, **When** `check_pending_reviews()` is called, **Then** it returns `Ok(None)` (graceful degradation, same as board unavailable), and the auto-dev loop falls through to regular board tasks.

7. **Given** a PR fix task is constructed from a `changes_requested` PR, **When** the task is represented as an `Assignment`, **Then** the `Assignment` has: `id` set to `"pr-fix-{pr_number}"`, `title` set to `"Fix PR #{pr_number}: {pr_title}"`, `status` set to `"ready-for-dev"`, `workflow_id` set to `"coding-pr-fix"`, `priority` set to `"critical"` (higher than any board task), and `labels` containing `"pr-fix"`.

## Tasks / Subtasks

- [ ] Task 1: Add `request_reviewers()` method to `GitHubClient` (AC: 3)
  - [ ] 1.1 Implement `fn request_reviewers(&self, pr_number: u64, reviewers: &[String]) -> Result<(), WitPluginError>` on `GitHubClient`
  - [ ] 1.2 Build URL: `{api_base}/repos/{owner}/{repo}/pulls/{pr_number}/requested_reviewers`
  - [ ] 1.3 Send `POST` with JSON body: `{"reviewers": ["user1", "user2"]}`
  - [ ] 1.4 Check response status: 201 Created = success, else return `WitPluginError::internal` with the error message
  - [ ] 1.5 Add `tracing::info!` on success: `"Re-requested review from {N} reviewers for PR #{pr_number}"`

- [ ] Task 2: Add `check_pending_reviews()` function in `src/auto_dev.rs` (AC: 1, 4, 5, 6, 7)
  - [ ] 2.1 Implement `fn check_pending_reviews(config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError>`
  - [ ] 2.2 Attempt to create `GitHubClient::new()` -- if it fails (no token), return `Ok(None)` (graceful degradation)
  - [ ] 2.3 Call `client.list_open_prs()` to get all open PRs
  - [ ] 2.4 Filter to auto-dev PRs using `is_auto_dev_pr()` from Story 23.1
  - [ ] 2.5 For each auto-dev PR, call `client.list_pr_reviews(pr.number)` and compute aggregate state using `aggregate_review_state()` from Story 23.1
  - [ ] 2.6 Filter to PRs with `changes_requested` state -- skip `approved` and `pending`
  - [ ] 2.7 If multiple PRs need fixes, pick the one with the lowest PR number (oldest first -- FIFO)
  - [ ] 2.8 Construct an `Assignment` from the PR:
    ```rust
    Assignment {
        id: format!("pr-fix-{}", pr.number),
        title: format!("Fix PR #{}: {}", pr.number, pr.title),
        status: "ready-for-dev".to_string(),
        workflow_id: "coding-pr-fix".to_string(),
        priority: "critical".to_string(),
        labels: vec!["pr-fix".to_string()],
        description: format!("PR #{} has changes requested. Branch: {}", pr.number, pr.head.ref_field),
        assignee: String::new(),
    }
    ```
  - [ ] 2.9 Return `Ok(Some(assignment))` or `Ok(None)` if no PRs need fixes

- [ ] Task 3: Modify `pick_next_task()` to check PR reviews first (AC: 1)
  - [ ] 3.1 At the top of `pick_next_task()`, call `check_pending_reviews(config)?`
  - [ ] 3.2 If `Some(assignment)` is returned, return it immediately (PR fixes have priority)
  - [ ] 3.3 If `None`, fall through to existing `board_client::list_assignments()` logic

- [ ] Task 4: Add PR metadata to workflow execution context (AC: 2)
  - [ ] 4.1 In `auto_dev_next()`, detect PR fix tasks by checking `task.id.starts_with("pr-fix-")`
  - [ ] 4.2 Extract `pr_number` from the task ID: parse the number after `"pr-fix-"`
  - [ ] 4.3 Extract `pr_branch` from the task description (parse from `"Branch: {branch}"`)
  - [ ] 4.4 Build enhanced `user_input` that includes PR metadata for template variable resolution:
    ```rust
    if task.id.starts_with("pr-fix-") {
        let pr_number = task.id.strip_prefix("pr-fix-").unwrap_or("0");
        // Extract branch from description "PR #N has changes requested. Branch: <branch>"
        let branch = task.description
            .split("Branch: ")
            .nth(1)
            .unwrap_or("unknown");
        format!(
            "pr_number={}\npr_branch={}\n\n{}",
            pr_number, branch, task.title
        )
    } else {
        // existing logic
        if task.description.is_empty() {
            task.title.clone()
        } else {
            format!("{}\n\n{}", task.title, task.description)
        }
    }
    ```
  - [ ] 4.5 Ensure `resolve_workflow_id()` returns `"coding-pr-fix"` for PR fix tasks (the explicit `workflow_id` field on the Assignment takes priority -- this is already handled by the existing `if !assignment.workflow_id.is_empty()` check at line 30-32 of `auto_dev.rs`)

- [ ] Task 5: Add re-request review after successful push (AC: 3)
  - [ ] 5.1 In `auto_dev_next()`, after the workflow succeeds and tests pass (the success branch at line 169), add a PR-fix-specific post-step
  - [ ] 5.2 Detect PR fix task: `if task.id.starts_with("pr-fix-")`
  - [ ] 5.3 Parse `pr_number` from task ID
  - [ ] 5.4 Create `GitHubClient::new()` and call `client.get_pull_request(pr_number)` to get the list of `requested_reviewers`
  - [ ] 5.5 If the PR has previous reviewers, extract reviewer logins from the reviews (get unique users who left `CHANGES_REQUESTED` reviews)
  - [ ] 5.6 Call `client.request_reviewers(pr_number, &reviewer_logins)?`
  - [ ] 5.7 If `request_reviewers` fails, log a warning but do NOT fail the entire auto-dev result -- the fix commits are already pushed
  - [ ] 5.8 Add board comment: `"[auto-dev] PR fix pushed. Re-requested review from: {reviewers}"`

- [ ] Task 6: Handle board status updates for PR fix tasks (AC: 2)
  - [ ] 6.1 PR fix tasks have synthetic IDs like `"pr-fix-42"` that do NOT exist in the board plugin
  - [ ] 6.2 In `auto_dev_next()`, wrap the `board_client::update_assignment()` and `board_client::add_comment()` calls with a guard: if the task ID starts with `"pr-fix-"`, skip board updates (or catch and log the error)
  - [ ] 6.3 The board status tracking for PR fix tasks is informational only -- the real state is tracked via GitHub PR review status
  - [ ] 6.4 Alternative approach: if the original board task can be identified (by matching PR branch to a board task), update that task instead. For v1, skip board updates for PR fix tasks and log the action.

- [ ] Task 7: Write unit tests (AC: 1, 4, 5, 6, 7)
  - [ ] 7.1 `test_check_pending_reviews_returns_none_without_token` -- verify graceful `Ok(None)` when GITHUB_TOKEN is unset
  - [ ] 7.2 `test_pr_fix_assignment_construction` -- verify `Assignment` fields: id format, title format, priority is "critical", workflow_id is "coding-pr-fix"
  - [ ] 7.3 `test_pr_fix_task_has_higher_priority_than_board` -- verify `priority_rank("critical") < priority_rank("high")`
  - [ ] 7.4 `test_pick_next_task_prioritizes_pr_fixes` -- mock scenario: verify PR fix returned before board task (unit test with constructed assignments, no HTTP)
  - [ ] 7.5 `test_detect_pr_fix_task_by_id_prefix` -- verify `task.id.starts_with("pr-fix-")` detection
  - [ ] 7.6 `test_parse_pr_number_from_task_id` -- verify extraction of `42` from `"pr-fix-42"`
  - [ ] 7.7 `test_parse_branch_from_description` -- verify extraction of branch name from description format
  - [ ] 7.8 `test_skip_approved_prs` -- verify PRs with `approved` state are not returned as fix tasks
  - [ ] 7.9 `test_skip_pending_prs` -- verify PRs with `pending` state (no reviews) are not returned
  - [ ] 7.10 `test_fifo_ordering_for_multiple_fix_prs` -- verify lowest PR number is picked first

- [ ] Task 8: Add `use` imports in `auto_dev.rs` (AC: all)
  - [ ] 8.1 Add `#[cfg(not(target_arch = "wasm32"))]` gated import for `crate::github_client::GitHubClient`
  - [ ] 8.2 Ensure the `check_pending_reviews()` function is also gated with `#[cfg(not(target_arch = "wasm32"))]` since it uses `GitHubClient`
  - [ ] 8.3 For WASM builds, provide a stub `check_pending_reviews()` that always returns `Ok(None)`

## Dev Notes

### Architecture: Two-Source Task Selection

After this story, `pick_next_task()` checks two sources in priority order:

```
1. check_pending_reviews() -> PR fix tasks (priority: critical)
2. board_client::list_assignments("ready-for-dev") -> regular board tasks (priority: varies)
```

PR fix tasks always win because they are constructed with `priority: "critical"`, and they are checked first. This ensures review feedback is addressed promptly.

```rust
pub fn pick_next_task(config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    // Priority 1: PR review fixes
    if let Some(pr_fix) = check_pending_reviews(config)? {
        return Ok(Some(pr_fix));
    }

    // Priority 2: Board tasks (existing logic)
    let assignments = match board_client::list_assignments(Some("ready-for-dev")) {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let ready = assignments
        .into_iter()
        .min_by_key(|a| priority_rank(&a.priority));
    Ok(ready)
}
```

### Graceful Degradation Pattern

The `check_pending_reviews()` function MUST be resilient to failure. If `GITHUB_TOKEN` is not set, or the GitHub API is unreachable, or any error occurs, it returns `Ok(None)` -- never `Err`. This matches the board unavailability pattern already in `pick_next_task()`:

```rust
fn check_pending_reviews(config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    // Graceful: if no GitHub token, skip PR review checking entirely
    let client = match crate::github_client::GitHubClient::new() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let prs = match client.list_open_prs() {
        Ok(prs) => prs,
        Err(e) => {
            tracing::warn!(plugin = "coding-pack", "Failed to list open PRs: {e}");
            return Ok(None);
        }
    };

    // ... filter and construct assignment ...
}
```

### Re-Request Review: GitHub API Call

The `POST /repos/{owner}/{repo}/pulls/{pr_number}/requested_reviewers` endpoint requires:

```json
{
  "reviewers": ["username1", "username2"]
}
```

Response: `201 Created` on success.

The reviewer list comes from the reviews on the PR (users who left `CHANGES_REQUESTED` reviews). Extract unique logins:

```rust
fn get_change_requesters(reviews: &[PrReview]) -> Vec<String> {
    let mut reviewers: Vec<String> = reviews.iter()
        .filter(|r| r.state == "CHANGES_REQUESTED")
        .map(|r| r.user.login.clone())
        .collect();
    reviewers.sort();
    reviewers.dedup();
    reviewers
}
```

### Implementation of `request_reviewers()` on GitHubClient

```rust
impl GitHubClient {
    pub fn request_reviewers(
        &self,
        pr_number: u64,
        reviewers: &[String],
    ) -> Result<(), WitPluginError> {
        if reviewers.is_empty() {
            return Ok(());
        }

        let url = format!(
            "{}/repos/{}/{}/pulls/{}/requested_reviewers",
            self.api_base, self.owner, self.repo, pr_number
        );

        let body = serde_json::json!({ "reviewers": reviewers });

        let resp = self.client
            .post(&url)
            .bearer_auth(&self.token)
            .header("User-Agent", "pulse-auto-dev")
            .json(&body)
            .send()
            .map_err(|e| github_err(format!("POST {url}: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(github_err(format!(
                "request_reviewers: HTTP {status} for PR #{pr_number}: {body}"
            )));
        }

        tracing::info!(
            plugin = "coding-pack",
            pr_number = pr_number,
            reviewers = ?reviewers,
            "Re-requested review"
        );
        Ok(())
    }
}
```

### Board Update Guard for Synthetic Tasks

PR fix tasks have IDs like `"pr-fix-42"` that don't exist in the board plugin. Board API calls will fail for these. Guard them:

```rust
// In auto_dev_next(), wrap board calls:
let is_pr_fix = task.id.starts_with("pr-fix-");

if !is_pr_fix {
    board_client::update_assignment(&task_id, &serde_json::json!({"status": "in-progress"}))?;
    board_client::add_comment(&task_id, &format!("[auto-dev] Starting workflow '{workflow_id}'"), "auto-dev")?;
} else {
    tracing::info!(plugin = "coding-pack", task_id = %task_id, "PR fix task -- skipping board status update");
}
```

Apply the same guard to all `board_client::update_assignment()` and `board_client::add_comment()` calls in `auto_dev_next()`. There are 5 board calls total in the function (lines 141-148, 171-179, 189-193, 206-210) -- all need the guard.

### WASM Compilation Gate

`auto_dev.rs` currently compiles for all targets. Since `check_pending_reviews()` uses `GitHubClient` (which requires `reqwest::blocking`), it must be gated:

```rust
#[cfg(not(target_arch = "wasm32"))]
fn check_pending_reviews(_config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    // ... full implementation ...
}

#[cfg(target_arch = "wasm32")]
fn check_pending_reviews(_config: &WorkspaceConfig) -> Result<Option<Assignment>, WitPluginError> {
    Ok(None)
}
```

### Error Handling in Post-Push Re-Request

The re-request review call happens AFTER the fix workflow succeeds and commits are pushed. If it fails, we must NOT fail the entire `auto_dev_next()` result. The fix is already applied. Log and continue:

```rust
// After successful push, re-request review (best-effort)
if is_pr_fix {
    if let Err(e) = re_request_review_for_pr(&task_id) {
        tracing::warn!(
            plugin = "coding-pack",
            task_id = %task_id,
            error = %e,
            "Failed to re-request review -- fix commits were pushed successfully"
        );
    }
}
```

### Anti-Patterns to Avoid

- **Do NOT** make `check_pending_reviews()` return `Err` on GitHub API failure -- always return `Ok(None)` for graceful degradation
- **Do NOT** fail `auto_dev_next()` if re-request review fails -- the fix is already pushed
- **Do NOT** call `board_client::update_assignment()` with synthetic `"pr-fix-{N}"` IDs -- these tasks don't exist in the board
- **Do NOT** use async reqwest -- `reqwest::blocking` only
- **Do NOT** log `GITHUB_TOKEN` -- ever
- **Do NOT** `unwrap()` or `expect()` in production code
- **Do NOT** add an infinite polling loop in `auto_dev_next()` -- it runs one cycle; `auto_dev_watch()` handles repeated execution
- **Do NOT** change the signature of `pick_next_task()` or `auto_dev_next()` -- they are public API; extend internally only
- **Do NOT** create new module files -- all changes go in existing `auto_dev.rs` and `github_client.rs`

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests`):

The `check_pending_reviews()` function makes HTTP calls, so direct unit testing is limited. Focus on:
1. Assignment construction logic (given PR data, verify correct Assignment fields)
2. PR number parsing from task ID
3. Branch parsing from description
4. FIFO ordering (lowest PR number first)
5. Graceful degradation (no token -> Ok(None))
6. Priority ranking (critical < high)

For tests that need `GitHubClient`, use the `#[cfg(test)]` pattern to test the helper functions that don't require HTTP:
```rust
#[cfg(test)]
fn construct_pr_fix_assignment(pr_number: u64, title: &str, branch: &str) -> Assignment {
    // ... same logic as check_pending_reviews() but without HTTP
}
```

**Integration tests** (require live GitHub API):
- Full cycle: detect `changes_requested` PR, construct task, run workflow -- mark with `#[ignore]`

### Files Modified

- `src/github_client.rs` -- add `request_reviewers()` method on `GitHubClient`
- `src/auto_dev.rs` -- add `check_pending_reviews()` function; modify `pick_next_task()` to check PR reviews first; modify `auto_dev_next()` to handle PR fix tasks (board guard, PR metadata extraction, re-request review); add WASM stubs; add unit tests

### No New Dependencies Needed

All required crates already in `Cargo.toml`. `std::collections::BTreeMap` is from standard library.

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md, Epic 23, Story 23.4]
- [Source: src/auto_dev.rs -- pick_next_task(), auto_dev_next(), resolve_workflow_id(), board_client usage]
- [Source: src/github_client.rs -- GitHubClient, list_open_prs(), list_pr_reviews(), aggregate_review_state(), is_auto_dev_pr() from Stories 22.1/23.1]
- [Source: src/board_client.rs -- update_assignment(), add_comment() patterns]
- [Source: config/workflows/coding-pr-fix.yaml -- workflow template from Story 23.3]
- [GitHub REST API: Request reviewers](https://docs.github.com/en/rest/pulls/review-requests#request-reviewers-for-a-pull-request)

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
