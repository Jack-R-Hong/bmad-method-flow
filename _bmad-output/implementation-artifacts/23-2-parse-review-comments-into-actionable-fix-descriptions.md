# Story 23.2: Parse Review Comments into Actionable Fix Descriptions

Status: ready-for-dev

## Story

As an operator,
I want the system to extract actionable fix instructions from reviewer comments,
So that the fix workflow receives clear, structured context for what needs to change.

## Acceptance Criteria

1. **Given** a PR has review state `changes_requested`, **When** `build_fix_context(pr_number)` is called on `GitHubClient`, **Then** the result is a `FixContext` struct containing: `pr_number: u64`, `branch: String`, `base_branch: String`, `html_url: String`, `review_summary: String` (concatenated bodies from reviews with `CHANGES_REQUESTED` state), and `file_comments: Vec<FileCommentGroup>`.

2. **Given** inline review comments exist on specific files/lines, **When** the fix context is built, **Then** each `FileCommentGroup` contains: `file_path: String` and `comments: Vec<InlineComment>` where each `InlineComment` has `line_number: Option<u32>`, `diff_hunk: String` (surrounding code context from GitHub), `reviewer_comment: String`, and `reviewer: String`. Comments are grouped by `file_path` and ordered by `line_number` (ascending, `None` sorts first).

3. **Given** a general review body (not inline) contains change requests, **When** the fix context is built, **Then** the `review_summary` field contains the concatenated bodies of all `CHANGES_REQUESTED` reviews, separated by `"\n\n---\n\n"`, with each prefixed by `"Reviewer: {login}\n"`.

4. **Given** multiple reviewers have left comments, **When** the fix context is built, **Then** all comments from all reviewers are included in `file_comments`, grouped by file path (not by reviewer), ordered by line number within each group.

5. **Given** `build_fix_context(pr_number)` is called, **When** the PR has no reviews or only `APPROVED`/`COMMENTED` reviews, **Then** an empty `FixContext` is returned with `review_summary: ""` and `file_comments: vec![]` (no error -- the caller decides whether to proceed).

6. **Given** `build_fix_context(pr_number)` is called, **When** the result is serialized to JSON, **Then** the output is valid JSON suitable for injection into a workflow step's `context_from` parameter.

## Tasks / Subtasks

- [ ] Task 1: Define `FixContext` and related types in `src/github_client.rs` (AC: 1, 2, 6)
  - [ ] 1.1 Define `FixContext` struct with `#[derive(Debug, Clone, Serialize, Deserialize)]`: `pr_number: u64`, `branch: String`, `base_branch: String`, `html_url: String`, `review_summary: String`, `file_comments: Vec<FileCommentGroup>`
  - [ ] 1.2 Define `FileCommentGroup` struct: `file_path: String`, `comments: Vec<InlineComment>`
  - [ ] 1.3 Define `InlineComment` struct: `line_number: Option<u32>`, `diff_hunk: String`, `reviewer_comment: String`, `reviewer: String`

- [ ] Task 2: Implement `build_fix_context(pr_number)` on `GitHubClient` (AC: 1, 2, 3, 4, 5)
  - [ ] 2.1 Call `self.list_pr_reviews(pr_number)?` to get all reviews
  - [ ] 2.2 Call `self.get_review_comments(pr_number)?` to get all inline comments
  - [ ] 2.3 Fetch the PR details to get `head.ref` (branch) and `base.ref` (base branch) -- call a new `fn get_pull_request(&self, pr_number: u64) -> Result<PullRequest, WitPluginError>` that GETs `/repos/{owner}/{repo}/pulls/{pr_number}`
  - [ ] 2.4 Build `review_summary` from reviews with `state == "CHANGES_REQUESTED"`:
    ```
    Reviewer: alice
    Please refactor the error handling to use the new pattern.

    ---

    Reviewer: bob
    The test coverage is insufficient for the edge case.
    ```
  - [ ] 2.5 Build `file_comments` by grouping `PrReviewComment` entries by `path`:
    - Create a `BTreeMap<String, Vec<InlineComment>>` for natural sort order by file path
    - Map each `PrReviewComment` to `InlineComment { line_number: comment.line, diff_hunk: comment.diff_hunk, reviewer_comment: comment.body, reviewer: comment.user.login }`
    - Within each file group, sort by `line_number` (ascending, `None` before `Some`)
  - [ ] 2.6 Assemble and return `FixContext`
  - [ ] 2.7 If no `CHANGES_REQUESTED` reviews and no inline comments, return a `FixContext` with empty `review_summary` and empty `file_comments`

- [ ] Task 3: Implement `get_pull_request(pr_number)` on `GitHubClient` (AC: 1)
  - [ ] 3.1 Build URL: `{api_base}/repos/{owner}/{repo}/pulls/{pr_number}`
  - [ ] 3.2 Parse response as `PullRequest`
  - [ ] 3.3 This is a single-object GET -- no pagination needed

- [ ] Task 4: Add `build-fix-context` action to `src/pack.rs` (AC: 6)
  - [ ] 4.1 Add match arm: `"build-fix-context" => build_fix_context_value(&config, input.payload.as_ref())`
  - [ ] 4.2 Extract `pr_number` from `input.payload` (required field): `payload.get("pr_number").and_then(|v| v.as_u64())`
  - [ ] 4.3 Return error if `pr_number` missing: `WitPluginError::invalid_input("build-fix-context requires 'pr_number' in payload")`
  - [ ] 4.4 Call `GitHubClient::new()?.build_fix_context(pr_number)?`
  - [ ] 4.5 Serialize `FixContext` to JSON via `serde_json::to_value()`
  - [ ] 4.6 Update the `other =>` error message to include `"build-fix-context"` in available actions

- [ ] Task 5: Write unit tests (AC: 1, 2, 3, 4, 5)
  - [ ] 5.1 `test_fix_context_serialization_roundtrip` -- create a `FixContext` with sample data, serialize to JSON, deserialize back, assert equality
  - [ ] 5.2 `test_review_summary_formatting` -- verify multi-reviewer summary format with `"---"` separator
  - [ ] 5.3 `test_file_comments_grouped_by_path` -- verify comments on same file are grouped together
  - [ ] 5.4 `test_file_comments_sorted_by_line` -- verify comments within a file are sorted by line number ascending
  - [ ] 5.5 `test_none_line_sorts_first` -- verify comments with `line_number: None` appear before those with `Some`
  - [ ] 5.6 `test_empty_fix_context_when_no_changes_requested` -- verify empty context when all reviews are APPROVED
  - [ ] 5.7 `test_multiple_reviewers_merged_by_file` -- verify comments from different reviewers on the same file appear in the same `FileCommentGroup`
  - [ ] 5.8 In `src/pack.rs`: `test_build_fix_context_action_recognized` -- verify action is in the dispatch table (returns `invalid_input` for missing pr_number, not `not_found`)

## Dev Notes

### Dependency: Story 23.1 Must Be Complete

This story depends on `list_pr_reviews()`, `get_review_comments()`, and `PrReview`/`PrReviewComment`/`PullRequest` types from Story 23.1. All of these must exist in `src/github_client.rs` before starting.

### Core Algorithm: Comment Grouping and Sorting

The key logic is transforming a flat `Vec<PrReviewComment>` into grouped-and-sorted `Vec<FileCommentGroup>`. Use `BTreeMap` for deterministic file path ordering:

```rust
impl GitHubClient {
    pub fn build_fix_context(&self, pr_number: u64) -> Result<FixContext, WitPluginError> {
        let pr = self.get_pull_request(pr_number)?;
        let reviews = self.list_pr_reviews(pr_number)?;
        let comments = self.get_review_comments(pr_number)?;

        // Build review_summary from CHANGES_REQUESTED reviews
        let review_summary = reviews.iter()
            .filter(|r| r.state == "CHANGES_REQUESTED")
            .filter_map(|r| {
                r.body.as_ref().filter(|b| !b.is_empty()).map(|body| {
                    format!("Reviewer: {}\n{}", r.user.login, body)
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        // Group inline comments by file path
        let mut by_file: std::collections::BTreeMap<String, Vec<InlineComment>> =
            std::collections::BTreeMap::new();

        for c in &comments {
            by_file.entry(c.path.clone()).or_default().push(InlineComment {
                line_number: c.line,
                diff_hunk: c.diff_hunk.clone(),
                reviewer_comment: c.body.clone(),
                reviewer: c.user.login.clone(),
            });
        }

        // Sort comments within each file by line number
        let file_comments: Vec<FileCommentGroup> = by_file.into_iter()
            .map(|(file_path, mut comments)| {
                comments.sort_by_key(|c| c.line_number.unwrap_or(0));
                FileCommentGroup { file_path, comments }
            })
            .collect();

        Ok(FixContext {
            pr_number,
            branch: pr.head.ref_field.clone(),
            base_branch: pr.base.ref_field.clone(),
            html_url: pr.html_url.clone(),
            review_summary,
            file_comments,
        })
    }
}
```

### FixContext JSON Output Shape

The serialized output is consumed by workflow template variables. Here is the expected shape:

```json
{
  "pr_number": 42,
  "branch": "auto-dev/story-23-2",
  "base_branch": "main",
  "html_url": "https://github.com/owner/repo/pull/42",
  "review_summary": "Reviewer: alice\nPlease refactor error handling.\n\n---\n\nReviewer: bob\nAdd more test coverage.",
  "file_comments": [
    {
      "file_path": "src/github_client.rs",
      "comments": [
        {
          "line_number": 45,
          "diff_hunk": "@@ -40,6 +40,10 @@ impl GitHubClient {\n     pub fn new() ...",
          "reviewer_comment": "This should handle the timeout case",
          "reviewer": "alice"
        },
        {
          "line_number": 78,
          "diff_hunk": "@@ -75,3 +75,8 @@ fn parse_link_next ...",
          "reviewer_comment": "Add a unit test for empty Link header",
          "reviewer": "bob"
        }
      ]
    },
    {
      "file_path": "src/pack.rs",
      "comments": [
        {
          "line_number": null,
          "diff_hunk": "@@ -0,0 +1,15 @@\n+fn check_pr_reviews ...",
          "reviewer_comment": "Missing error handling for 404",
          "reviewer": "alice"
        }
      ]
    }
  ]
}
```

### Action Registration Pattern

Follow the existing `"auto-dev-next"` pattern in `pack.rs` for actions that need payload data:

```rust
"build-fix-context" => {
    let pr_number = input.payload.as_ref()
        .and_then(|p| p.get("pr_number"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| WitPluginError::invalid_input(
            "build-fix-context requires 'pr_number' in payload"
        ))?;
    let client = crate::github_client::GitHubClient::new()?;
    let ctx = client.build_fix_context(pr_number)?;
    to_json_string(serde_json::to_value(&ctx)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}"))))
}
```

### Handling `None` Line Numbers

GitHub inline comments on the PR description or on deleted lines may have `line: null`. These are still valuable fix context. Sort them before numbered lines (treat as line 0):

```rust
comments.sort_by_key(|c| c.line_number.unwrap_or(0));
```

### Anti-Patterns to Avoid

- **Do NOT** filter out comments with `None` line numbers -- they contain valuable review context
- **Do NOT** group by reviewer -- group by file path for developer convenience (the dev fixes files, not reviewers)
- **Do NOT** skip reviews with `COMMENTED` state for inline comments -- `get_review_comments()` returns all inline comments regardless of review state; the `review_summary` filter is only for review body text
- **Do NOT** use `unwrap()` or `expect()` in production -- map all errors to `WitPluginError`
- **Do NOT** log PR body content at info/warn level -- review comments may contain sensitive code context; use `debug!` or `trace!` only
- **Do NOT** use `HashMap` -- use `BTreeMap` for deterministic ordering of file paths in output
- **Do NOT** create a separate module file -- all types and logic go in `src/github_client.rs`

### Testing Strategy

**Unit tests** (inline `#[cfg(test)] mod tests` in `src/github_client.rs`):
- Test the grouping/sorting logic with constructed `PrReviewComment` vectors -- no HTTP needed
- Test `review_summary` formatting with multiple CHANGES_REQUESTED reviews
- Test serialization roundtrip of `FixContext`
- Test edge case: empty comments list -> empty `file_comments`
- Test edge case: comments with `None` line numbers

**No integration tests for this story** -- the HTTP calls are tested in 23.1. This story's logic is pure data transformation that can be fully unit-tested.

### Files Modified

- `src/github_client.rs` -- add `FixContext`, `FileCommentGroup`, `InlineComment` types; add `build_fix_context()` and `get_pull_request()` methods on `GitHubClient`; add unit tests
- `src/pack.rs` -- add `"build-fix-context"` match arm in `execute_action()`; update error message for unknown actions

### No New Dependencies Needed

All required crates already in `Cargo.toml`. The `std::collections::BTreeMap` is from the standard library.

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md, Epic 23, Story 23.2]
- [Source: src/github_client.rs -- GitHubClient struct, PrReview, PrReviewComment types from Story 23.1]
- [Source: src/pack.rs -- execute_action() dispatch pattern, payload extraction pattern]
- [GitHub REST API: Get a pull request](https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request)

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
