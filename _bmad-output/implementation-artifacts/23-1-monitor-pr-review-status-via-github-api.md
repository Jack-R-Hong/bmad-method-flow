# Story 23.1: Monitor PR Review Status via GitHub API

Status: ready-for-dev

## Story

As an operator,
I want the system to check the review status of auto-dev PRs via GitHub API,
So that PRs with "changes requested" are detected and queued for automated fixes.

## Acceptance Criteria

1. **Given** `GitHubClient` exists from Story 22.1 (in `src/github_client.rs`), **When** `list_pr_reviews(pr_number)` is called, **Then** reviews are returned as `Vec<PrReview>` with fields: `id: u64`, `state: String` (one of `"APPROVED"`, `"CHANGES_REQUESTED"`, `"COMMENTED"`, `"DISMISSED"`, `"PENDING"`), `body: String`, `user: String`, `submitted_at: String`, and all reviews for that PR are included (paginated if > 100).

2. **Given** `get_review_comments(pr_number)` is called, **When** the PR has inline review comments, **Then** comments are returned as `Vec<PrReviewComment>` with fields: `id: u64`, `path: String`, `line: Option<u32>`, `body: String`, `diff_hunk: String`, `user: String`, `created_at: String`, and pagination is handled via the `Link` header.

3. **Given** `list_open_prs()` is called, **When** open PRs exist, **Then** PRs are returned as `Vec<PullRequest>` with fields: `number: u64`, `title: String`, `head_ref: String` (branch name), `base_ref: String`, `html_url: String`, `user: String`, `body: Option<String>`, `requested_reviewers: Vec<String>`.

4. **Given** a new action `check-pr-reviews` is added to `execute_action()` in `src/pack.rs`, **When** invoked with `{"action": "check-pr-reviews"}`, **Then** the system scans all open PRs created by auto-dev (identified by branch prefix `auto-dev/` OR body containing `Co-authored-by: pulse-auto-dev`), and returns a JSON array of objects with fields: `pr_number`, `title`, `branch`, `review_state` (one of `"approved"`, `"changes_requested"`, `"pending"`), `html_url`.

5. **Given** the polling interval is configurable via `github_sync.review_poll_interval_secs` in `config/config.yaml` (default: 60), **When** the config is loaded via `WorkspaceConfig`, **Then** the interval value is available as `config.github_sync.review_poll_interval_secs` for callers to read. The `check-pr-reviews` action itself does not loop -- it runs once and returns; callers are responsible for polling.

6. **Given** `GITHUB_TOKEN` is not set, **When** the `check-pr-reviews` action is invoked, **Then** an error is returned: `WitPluginError::invalid_input("GITHUB_TOKEN environment variable not set")`.

## Tasks / Subtasks

- [ ] Task 1: Add PR review types to `src/github_client.rs` (AC: 1, 2, 3)
  - [ ] 1.1 Define `PrReview` struct with `#[derive(Debug, Clone, Serialize, Deserialize)]`: `id: u64`, `state: String`, `body: Option<String>`, `user: GitHubUser`, `submitted_at: Option<String>`
  - [ ] 1.2 Define `PrReviewComment` struct: `id: u64`, `path: String`, `line: Option<u32>`, `body: String`, `diff_hunk: String`, `user: GitHubUser`, `created_at: String`
  - [ ] 1.3 Define `PullRequest` struct: `number: u64`, `title: String`, `head: PrRef`, `base: PrRef`, `html_url: String`, `user: GitHubUser`, `body: Option<String>`, `requested_reviewers: Vec<GitHubUser>`
  - [ ] 1.4 Define `PrRef` struct: `ref_field: String` (serde rename from `"ref"`), `sha: String`
  - [ ] 1.5 Define `GitHubUser` struct if not already present: `login: String`

- [ ] Task 2: Implement `list_pr_reviews(pr_number)` on `GitHubClient` (AC: 1)
  - [ ] 2.1 Build URL: `{api_base}/repos/{owner}/{repo}/pulls/{pr_number}/reviews`
  - [ ] 2.2 Add `Authorization: Bearer {token}` and `User-Agent: pulse-auto-dev` headers
  - [ ] 2.3 Parse response as `Vec<PrReview>`, handle pagination via `Link` header (reuse existing `parse_link_next()` helper from Story 22.1)
  - [ ] 2.4 Add `tracing::debug!` for page count and review count -- NEVER log the token

- [ ] Task 3: Implement `get_review_comments(pr_number)` on `GitHubClient` (AC: 2)
  - [ ] 3.1 Build URL: `{api_base}/repos/{owner}/{repo}/pulls/{pr_number}/comments`
  - [ ] 3.2 Parse response as `Vec<PrReviewComment>` with pagination
  - [ ] 3.3 The `diff_hunk` field comes directly from GitHub's API response -- no custom parsing needed

- [ ] Task 4: Implement `list_open_prs()` on `GitHubClient` (AC: 3)
  - [ ] 4.1 Build URL: `{api_base}/repos/{owner}/{repo}/pulls?state=open&per_page=100`
  - [ ] 4.2 Parse response as `Vec<PullRequest>` with pagination
  - [ ] 4.3 Add `tracing::debug!` log for number of open PRs found

- [ ] Task 5: Add `check-pr-reviews` action to `src/pack.rs` (AC: 4, 6)
  - [ ] 5.1 Add a new match arm in `execute_action()`: `"check-pr-reviews" => check_pr_reviews_value(&config)`
  - [ ] 5.2 Implement `fn check_pr_reviews_value(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError>`
  - [ ] 5.3 Instantiate `GitHubClient::new()` -- propagate error if GITHUB_TOKEN missing
  - [ ] 5.4 Call `list_open_prs()`, filter to auto-dev PRs: branch starts with `auto-dev/` OR body contains `Co-authored-by: pulse-auto-dev`
  - [ ] 5.5 For each matching PR, call `list_pr_reviews(pr.number)` and compute aggregate review state:
    - If any review has `state == "CHANGES_REQUESTED"` and no later `"APPROVED"` from same user -> `"changes_requested"`
    - If any review has `state == "APPROVED"` and none have `"CHANGES_REQUESTED"` after it -> `"approved"`
    - Otherwise -> `"pending"`
  - [ ] 5.6 Return JSON array: `[{ "pr_number": N, "title": "...", "branch": "...", "review_state": "...", "html_url": "..." }]`
  - [ ] 5.7 Update the `other =>` error message to include `"check-pr-reviews"` in the available actions list

- [ ] Task 6: Add `github_sync` config to `WorkspaceConfig` (AC: 5)
  - [ ] 6.1 Define `GitHubSyncConfig` struct in `src/workspace.rs`: `review_poll_interval_secs: u64` with `#[serde(default = "default_review_poll_interval")]` (default 60)
  - [ ] 6.2 Add `github_sync: GitHubSyncConfig` field to `WorkspaceConfig`
  - [ ] 6.3 Add `github_sync: Option<GitHubSyncConfig>` to `ConfigYaml` struct
  - [ ] 6.4 Wire it in `WorkspaceConfig::from_base_dir()`: `github_sync: yaml.github_sync.unwrap_or_default()`
  - [ ] 6.5 Update `WorkspaceConfig::default_for()` to include `github_sync: GitHubSyncConfig::default()`

- [ ] Task 7: Write unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] 7.1 `test_pr_review_deserialization` -- deserialize sample GitHub PR review JSON into `PrReview`
  - [ ] 7.2 `test_pr_review_comment_deserialization` -- deserialize sample inline comment JSON into `PrReviewComment`
  - [ ] 7.3 `test_pull_request_deserialization` -- deserialize sample PR JSON into `PullRequest`
  - [ ] 7.4 `test_aggregate_review_state_changes_requested` -- verify `changes_requested` beats older `approved`
  - [ ] 7.5 `test_aggregate_review_state_approved` -- verify `approved` with no subsequent `changes_requested`
  - [ ] 7.6 `test_aggregate_review_state_pending` -- verify `pending` when no reviews exist
  - [ ] 7.7 `test_github_sync_config_default` -- verify default `review_poll_interval_secs` is 60
  - [ ] 7.8 `test_github_sync_config_parsed` -- verify custom value from YAML
  - [ ] 7.9 `test_filter_auto_dev_prs_by_branch` -- verify PRs with `auto-dev/` prefix are matched
  - [ ] 7.10 `test_filter_auto_dev_prs_by_body` -- verify PRs with `Co-authored-by: pulse-auto-dev` in body are matched

## Dev Notes

### Dependency: Story 22.1 Must Be Complete

This story extends `src/github_client.rs` created in Story 22.1. The `GitHubClient` struct with `new()`, `token`, `owner`, `repo`, `client` fields, and the `parse_link_next()` pagination helper must already exist. If they don't, implement them first following the 22.1 spec.

### Critical Pattern: Follow Existing `GitHubClient` Method Style

Story 22.1 establishes the method pattern on `GitHubClient`. New methods (`list_pr_reviews`, `get_review_comments`, `list_open_prs`) must follow the same conventions:

```rust
// Pattern from 22.1 — method on GitHubClient struct
impl GitHubClient {
    pub fn list_pr_reviews(&self, pr_number: u64) -> Result<Vec<PrReview>, WitPluginError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/reviews",
            self.api_base, self.owner, self.repo, pr_number);

        let mut all_reviews = Vec::new();
        let mut next_url = Some(url);

        while let Some(url) = next_url {
            let resp = self.client.get(&url)
                .bearer_auth(&self.token)
                .header("User-Agent", "pulse-auto-dev")
                .send()
                .map_err(|e| github_err(format!("GET {url}: {e}")))?;

            let link_header = resp.headers()
                .get("link")
                .and_then(|v| v.to_str().ok())
                .map(String::from);

            let body = resp.text().map_err(|e| github_err(e))?;
            let page: Vec<PrReview> = serde_json::from_str(&body)
                .map_err(|e| github_err(format!("parse: {e}")))?;
            all_reviews.extend(page);

            next_url = link_header.as_deref().and_then(parse_link_next);
        }

        Ok(all_reviews)
    }
}
```

### GitHub REST API Endpoints

**List reviews for a PR:**
- `GET /repos/{owner}/{repo}/pulls/{pull_number}/reviews`
- Returns: array of review objects with `id`, `user`, `body`, `state`, `submitted_at`
- States: `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`, `DISMISSED`, `PENDING`

**List review comments (inline) for a PR:**
- `GET /repos/{owner}/{repo}/pulls/{pull_number}/comments`
- Returns: array of comment objects with `id`, `path`, `line`, `body`, `diff_hunk`, `user`, `created_at`
- The `diff_hunk` field is the surrounding diff context GitHub attaches to the comment

**List pull requests:**
- `GET /repos/{owner}/{repo}/pulls?state=open&per_page=100`
- Returns: array of PR objects with `number`, `title`, `head.ref`, `base.ref`, `html_url`, `user`, `body`, `requested_reviewers`
- The `head.ref` gives the branch name (e.g., `auto-dev/story-23-1`)

### Review State Aggregation Logic

GitHub returns all reviews chronologically. The aggregate state for a PR must account for review history:

```rust
fn aggregate_review_state(reviews: &[PrReview]) -> &'static str {
    // Group reviews by user, keep only latest per user
    let mut latest_by_user: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    for review in reviews {
        // Only consider APPROVED and CHANGES_REQUESTED (ignore COMMENTED, DISMISSED, PENDING)
        match review.state.as_str() {
            "APPROVED" | "CHANGES_REQUESTED" => {
                latest_by_user.insert(&review.user.login, &review.state);
            }
            _ => {}
        }
    }

    if latest_by_user.is_empty() {
        return "pending";
    }
    if latest_by_user.values().any(|s| *s == "CHANGES_REQUESTED") {
        return "changes_requested";
    }
    "approved"
}
```

### Auto-Dev PR Detection

PRs created by the auto-dev loop are identified by two signals (OR logic):
1. Branch name starts with `auto-dev/` (e.g., `auto-dev/story-23-1`)
2. PR body contains `Co-authored-by: pulse-auto-dev`

Check both because the branch prefix is the primary signal, but the body tag is a fallback for PRs created with custom branch names.

```rust
fn is_auto_dev_pr(pr: &PullRequest) -> bool {
    let branch_match = pr.head.ref_field.starts_with("auto-dev/");
    let body_match = pr.body.as_deref()
        .map(|b| b.contains("Co-authored-by: pulse-auto-dev"))
        .unwrap_or(false);
    branch_match || body_match
}
```

### WorkspaceConfig Extension Pattern

Follow the exact pattern from `AutoDevConfig` in `src/workspace.rs`:

```rust
// In workspace.rs
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubSyncConfig {
    #[serde(default = "default_review_poll_interval")]
    pub review_poll_interval_secs: u64,
}

impl Default for GitHubSyncConfig {
    fn default() -> Self {
        Self {
            review_poll_interval_secs: default_review_poll_interval(),
        }
    }
}

fn default_review_poll_interval() -> u64 {
    60
}
```

Add to `WorkspaceConfig`:
```rust
pub struct WorkspaceConfig {
    // ... existing fields ...
    pub github_sync: GitHubSyncConfig,
}
```

Add to `ConfigYaml`:
```rust
struct ConfigYaml {
    // ... existing fields ...
    #[serde(default)]
    github_sync: Option<GitHubSyncConfig>,
}
```

### Action Registration Pattern in `pack.rs`

Follow the existing `auto-dev-status` / `auto-dev-next` pattern in `execute_action()`:

```rust
// In pack.rs execute_action() match block, add:
"check-pr-reviews" => {
    let result = check_pr_reviews_value(&config)?;
    to_json_string(Ok(result))
}
```

The `check_pr_reviews_value` function lives in `pack.rs` (not `auto_dev.rs`) because it's a standalone query action, not part of the auto-dev loop. The auto-dev integration comes in Story 23.4.

### Serde Rename for `ref` Field

GitHub's PR JSON uses `"ref"` as a field name, which is a Rust keyword. Use serde rename:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrRef {
    #[serde(rename = "ref")]
    pub ref_field: String,
    pub sha: String,
}
```

### Anti-Patterns to Avoid

- **Do NOT** use async reqwest -- use `reqwest::blocking` only (no async runtime in production)
- **Do NOT** add polling/looping in the `check-pr-reviews` action -- it runs once; callers poll
- **Do NOT** log `GITHUB_TOKEN` at any level -- use `tracing` but sanitize token from all messages
- **Do NOT** `unwrap()` or `expect()` in production code -- always map to `WitPluginError`
- **Do NOT** hardcode `github.com` -- use `GITHUB_API_URL` env var (already set up in 22.1 client)
- **Do NOT** create new files beyond what's specified -- extend existing `github_client.rs`, `pack.rs`, `workspace.rs`
- **Do NOT** use `HashMap` for serialized output -- use `BTreeMap` if ordering matters
- **Do NOT** use `println!` / `eprintln!` -- use `tracing` macros only

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests`):
- In `src/github_client.rs`: test deserialization of review, comment, and PR JSON fixtures
- In `src/github_client.rs`: test `aggregate_review_state()` logic with various review histories
- In `src/github_client.rs`: test `is_auto_dev_pr()` matching logic
- In `src/workspace.rs`: test `GitHubSyncConfig` default and parsed values
- In `src/pack.rs`: test that `check-pr-reviews` action is recognized (no longer returns `not_found`)

**Integration tests** (future, requires live GitHub API):
- Call `list_pr_reviews()` on a real PR -- mark with `#[ignore]`
- Require `GITHUB_TOKEN` env var

### Files Modified

- `src/github_client.rs` -- add `PrReview`, `PrReviewComment`, `PullRequest`, `PrRef`, `GitHubUser` types; add `list_pr_reviews()`, `get_review_comments()`, `list_open_prs()` methods; add `aggregate_review_state()` and `is_auto_dev_pr()` helpers; add unit tests
- `src/pack.rs` -- add `"check-pr-reviews"` match arm in `execute_action()`; add `check_pr_reviews_value()` function; update error message for unknown actions
- `src/workspace.rs` -- add `GitHubSyncConfig` struct; add `github_sync` field to `WorkspaceConfig` and `ConfigYaml`; add unit tests for config parsing

### No New Dependencies Needed

All required crates are already in `Cargo.toml`:
- `reqwest 0.12` (blocking + json) -- HTTP client
- `serde 1.0` (derive) -- serialization
- `serde_json 1.0` -- JSON parsing
- `tracing 0.1` -- structured logging

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md, Epic 23, Story 23.1]
- [Source: src/github_client.rs -- GitHubClient struct and method pattern from Story 22.1]
- [Source: src/board_client.rs -- HTTP client pattern]
- [Source: src/pack.rs -- execute_action() dispatch pattern]
- [Source: src/workspace.rs -- WorkspaceConfig, AutoDevConfig pattern for new config sections]
- [Source: config/plugins/plugin-git-pr -- GitHub API curl patterns for PR operations]
- [GitHub REST API: Pull Request Reviews](https://docs.github.com/en/rest/pulls/reviews)
- [GitHub REST API: Review Comments](https://docs.github.com/en/rest/pulls/comments)
- [GitHub REST API: Pull Requests](https://docs.github.com/en/rest/pulls/pulls)

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
