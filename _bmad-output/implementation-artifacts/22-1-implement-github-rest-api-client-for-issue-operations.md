# Story 22.1: Implement GitHub REST API Client for Issue Operations

Status: review

## Story

As a plugin developer,
I want a `GitHubClient` struct in `src/github_client.rs` that authenticates via `GITHUB_TOKEN` and performs GitHub REST API operations,
So that the plugin has a reusable, tested foundation for all GitHub API interactions needed by the auto-dev loop.

## Acceptance Criteria

1. **Given** `GITHUB_TOKEN` environment variable is set, **When** `GitHubClient::new()` is called, **Then** the client stores the token for Bearer authentication in all API requests and parses `owner/repo` from `git remote get-url origin` (supporting both SSH and HTTPS remote formats).

2. **Given** a valid repo is detected, **When** `list_issues(state, labels, milestone)` is called, **Then** issues are returned as `Vec<GitHubIssue>` with fields: `number`, `title`, `body`, `labels`, `milestone`, `html_url`, and pagination is handled via GitHub's `Link: <next>` header, fetching all pages up to 1000 issues.

3. **Given** `GITHUB_TOKEN` is not set, **When** `GitHubClient::new()` is called, **Then** an error is returned: `WitPluginError::invalid_input("GITHUB_TOKEN environment variable not set")`.

4. **Given** the GitHub API responds slower than 5 seconds, **When** the request times out, **Then** a timeout error is returned without hanging the process.

5. **Given** the client uses `reqwest::blocking` (matching existing `board_client.rs` pattern), **When** `cargo clippy -- -D warnings` is run, **Then** no warnings or errors are reported for `src/github_client.rs`.

## Tasks / Subtasks

- [x] Task 1: Create `src/github_client.rs` module with types (AC: 1, 2)
  - [x] 1.1 Define `GitHubClient` struct with `token: String`, `owner: String`, `repo: String`, `client: reqwest::blocking::Client`
  - [x] 1.2 Define `GitHubIssue` struct with serde Deserialize: `number: u64`, `title: String`, `body: Option<String>`, `labels: Vec<GitHubLabel>`, `milestone: Option<GitHubMilestone>`, `html_url: String`, `state: String`
  - [x] 1.3 Define `GitHubLabel` struct: `name: String`
  - [x] 1.4 Define `GitHubMilestone` struct: `title: String`, `number: u64`
  - [x] 1.5 Add `pub mod github_client;` to `src/lib.rs` (behind `#[cfg(not(target_arch = "wasm32"))]`)

- [x] Task 2: Implement `GitHubClient::new()` constructor (AC: 1, 3)
  - [x] 2.1 Read `GITHUB_TOKEN` from env, return `WitPluginError::invalid_input` if missing
  - [x] 2.2 Build `reqwest::blocking::Client` with 5-second timeout and `User-Agent: pulse-auto-dev` header
  - [x] 2.3 Parse `owner/repo` from `git remote get-url origin` via `std::process::Command`
  - [x] 2.4 Support both SSH (`git@github.com:owner/repo.git`) and HTTPS (`https://github.com/owner/repo.git`) formats
  - [x] 2.5 Read optional `GITHUB_API_URL` env var for GitHub Enterprise, default to `https://api.github.com`

- [x] Task 3: Implement `list_issues()` with pagination (AC: 2, 4)
  - [x] 3.1 Build URL: `https://api.github.com/repos/{owner}/{repo}/issues`
  - [x] 3.2 Add query params: `state`, `labels` (comma-separated), `milestone`, `per_page=100`
  - [x] 3.3 Add auth header: `Authorization: Bearer {token}`
  - [x] 3.4 Parse `Link` header for `rel="next"` URL to handle pagination
  - [x] 3.5 Accumulate pages, cap at 10 pages (1000 issues max)
  - [x] 3.6 Filter out pull requests (GitHub issues API includes PRs; filter by absence of `pull_request` field)

- [x] Task 4: Add helper error function and logging (AC: 5)
  - [x] 4.1 Create `fn github_err(msg: impl std::fmt::Display) -> WitPluginError` helper
  - [x] 4.2 Add tracing logs with `plugin = "coding-pack"` field — NEVER log the token value
  - [x] 4.3 Run `cargo clippy -- -D warnings` and `cargo fmt --check`

- [x] Task 5: Write unit tests (AC: 1, 2, 3)
  - [x] 5.1 `test_parse_github_remote_https` — parse HTTPS remote URL
  - [x] 5.2 `test_parse_github_remote_ssh` — parse SSH remote URL
  - [x] 5.3 `test_new_without_token_returns_error` — verify error when `GITHUB_TOKEN` unset
  - [x] 5.4 `test_parse_link_header_next` — extract next page URL from Link header
  - [x] 5.5 `test_parse_link_header_none` — return None when no next page
  - [x] 5.6 `test_filter_pull_requests_from_issues` — verify PRs (with `pull_request` field) are excluded from results

## Dev Notes

### Critical Pattern: Follow `board_client.rs` HTTP Client Pattern

The codebase already has an established HTTP client pattern in `src/board_client.rs` (131 lines). **Reuse this pattern exactly**:

```rust
// board_client.rs pattern — stateless functions with reqwest blocking
fn board_api(path: &str) -> String {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".into());
    format!("http://127.0.0.1:{port}/api/v1/plugins/plugin-board/data/{path}")
}

fn api_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("Board API error: {msg}"))
}

// GET with text body then serde parse
let body = reqwest::blocking::get(&url)
    .map_err(|e| api_err(format!("GET {url}: {e}")))?
    .text()
    .map_err(|e| api_err(e))?;
let resp: T = serde_json::from_str(&body)
    .map_err(|e| api_err(format!("parse: {e}")))?;
```

**Key difference for GitHubClient**: Unlike `board_client.rs` (stateless functions), use a **struct-based client** because GitHub requires an auth token and owner/repo context that should be initialized once and reused across calls. This matches the epic's requirement for `GitHubClient::new()`.

### Git Remote Parsing

The `plugin-git-pr` bash script at `config/plugins/plugin-git-pr` already parses remotes. Reference its `parse_github_repo()` function (lines 23-30) for the two formats:

```bash
# SSH: git@github.com:owner/repo.git → owner/repo
# HTTPS: https://github.com/owner/repo.git → owner/repo
```

In Rust, use `std::process::Command::new("git").args(["remote", "get-url", "origin"])` then parse with string operations or a simple regex. Handle `.git` suffix stripping.

### GitHub REST API v3 Specifics

- **Base URL**: `https://api.github.com` (support self-hosted via optional `GITHUB_API_URL` env var)
- **Auth header**: `Authorization: Bearer {token}` (not `token {token}` — Bearer is the current standard)
- **User-Agent required**: GitHub API rejects requests without `User-Agent` header
- **Rate limiting**: 5000 requests/hour for authenticated users; check `X-RateLimit-Remaining` header
- **Issues endpoint**: `GET /repos/{owner}/{repo}/issues` — returns both issues AND pull requests by default
- **Filter PRs**: GitHub issues response includes a `pull_request` key on PRs — filter these out with `serde_json::Value` check or an optional field
- **Pagination**: `Link` header with `rel="next"`, `rel="last"` — parse with string split, not a full HTTP header parser
- **Per-page max**: 100 items per page (`?per_page=100`)

### Link Header Pagination Pattern

```
Link: <https://api.github.com/repos/owner/repo/issues?page=2&per_page=100>; rel="next",
      <https://api.github.com/repos/owner/repo/issues?page=10&per_page=100>; rel="last"
```

Parse `rel="next"` URL: split on `,`, find the segment containing `rel="next"`, extract URL between `<` and `>`.

### Filtering Pull Requests from Issues Response

GitHub's `/issues` endpoint returns both issues and PRs. PRs have a `pull_request` field in the JSON response. Use a helper field in the deserialization:

```rust
#[derive(Deserialize)]
struct RawGitHubIssue {
    // ... all fields ...
    pull_request: Option<serde_json::Value>,  // present on PRs, absent on issues
}
```

Then filter: `raw_issues.into_iter().filter(|i| i.pull_request.is_none())`.

### Dependencies — Already Available

All required dependencies exist in `Cargo.toml`:
- `reqwest = { version = "0.12", features = ["blocking", "json"] }` — HTTP client
- `serde = { version = "1.0", features = ["derive"] }` — serialization
- `serde_json = "1.0"` — JSON parsing
- `tracing = "0.1"` — structured logging

**No new dependencies needed.**

### WASM Gate

`board_client.rs` uses `#[cfg(not(target_arch = "wasm32"))]` for the entire module. Apply the same gate to `github_client.rs` since reqwest blocking requires native runtime.

### Project Structure Notes

- **New file**: `src/github_client.rs` — flat module, no nested dirs
- **Modified file**: `src/lib.rs` — add `#[cfg(not(target_arch = "wasm32"))] pub mod github_client;`
- **No changes to**: `Cargo.toml`, `src/auto_dev.rs`, `src/workspace.rs` (those come in Stories 22.2-22.4)
- Module follows flat `src/*.rs` convention — no `mod.rs` pattern

### Error Handling Constraints

- All errors map to `WitPluginError` — use `WitPluginError::internal()` for API errors, `WitPluginError::invalid_input()` for missing config
- **NEVER** `unwrap()` or `expect()` in production code
- **NEVER** log `GITHUB_TOKEN` value at any level (including `trace!`)
- **NEVER** include token in error messages — sanitize before constructing error strings
- Error messages use format: `"GitHub API error: GET {url}: {e}"` (matches `board_client.rs` pattern)

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests`):
- Test remote URL parsing (SSH and HTTPS formats)
- Test Link header parsing
- Test error on missing GITHUB_TOKEN (use `std::env::remove_var` in test, restore after)
- Test PR filtering logic

**Integration tests** (future, Story 22.2+):
- Live GitHub API tests with `#[ignore]` attribute (matching `board_client_integration.rs` pattern)
- Require `GITHUB_TOKEN` env var to be set

### Logging Conventions

```rust
tracing::info!(plugin = "coding-pack", "initializing GitHub client for {owner}/{repo}");
tracing::debug!(plugin = "coding-pack", page = page_num, count = issues.len(), "fetched issues page");
tracing::warn!(plugin = "coding-pack", remaining = rate_limit, "GitHub API rate limit low");
// NEVER: tracing::debug!(token = %token, "using token");  // CREDENTIAL LEAK
```

### Anti-Patterns to Avoid

- **Do NOT** use async reqwest — use `reqwest::blocking` only (no async runtime in production)
- **Do NOT** create a global/static client — instantiate per-use via `GitHubClient::new()`
- **Do NOT** hard-code `github.com` — support `GITHUB_API_URL` env var for GitHub Enterprise
- **Do NOT** add internal retry loops — retry policy belongs to the dispatch engine
- **Do NOT** use `println!` or `eprintln!` — always use `tracing` macros
- **Do NOT** use `HashMap` for serialized output — use `BTreeMap` if generating JSON/YAML
- **Do NOT** hand-parse JSON with `.get().and_then()` — use typed serde deserialization

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md#Epic 22, Story 22.1]
- [Source: src/board_client.rs — HTTP client pattern to follow]
- [Source: config/plugins/plugin-git-pr — GitHub API + remote parsing reference]
- [Source: _bmad-output/planning-artifacts/architecture.md — Error handling, module structure, credential isolation]
- [Source: _bmad-output/planning-artifacts/prd.md — NFR5: API call within 5 seconds, NFR6: tokens in host config only]

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
N/A

### Completion Notes List
- Created `src/github_client.rs` with `GitHubClient` struct, types, constructor, `list_issues()` with pagination, PR filtering, link header parsing, git remote parsing
- Custom `Debug` impl on `GitHubClient` to redact token from debug output
- 11 unit tests: remote URL parsing (SSH/HTTPS), link header parsing, missing token error, PR filtering, error helper
- All 144 unit tests pass, clippy clean with `-D warnings`
- Fixed pre-existing clippy warnings in `board_client.rs` and `pulse_api.rs` (redundant closures, map_or -> is_none_or)

### File List
- `src/github_client.rs` (new) - GitHub REST API client module
- `src/lib.rs` (modified) - Added `pub mod github_client` behind WASM gate
- `src/board_client.rs` (modified) - Fixed pre-existing clippy warnings
- `src/pulse_api.rs` (modified) - Fixed pre-existing clippy warning

### Change Log
- 2026-03-27: Story 22-1 implemented and moved to review
