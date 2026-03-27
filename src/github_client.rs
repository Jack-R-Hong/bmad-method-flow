//! GitHub REST API client for issue and pull request operations.
//!
//! Provides a struct-based HTTP client authenticated via `GITHUB_TOKEN`
//! environment variable for fetching issues and PR reviews from a GitHub
//! repository. Uses `reqwest::blocking` (no async runtime).

use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};

// ── Error helper ────────────────────────────────────────────────────────

/// Create a `WitPluginError::internal` for GitHub API errors.
fn github_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("GitHub API error: {msg}"))
}

// ── Types ───────────────────────────────────────────────────────────────

/// A GitHub issue (excludes pull requests).
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub labels: Vec<GitHubLabel>,
    pub milestone: Option<GitHubMilestone>,
    pub html_url: String,
    pub state: String,
}

/// A GitHub issue label.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

/// A GitHub milestone.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubMilestone {
    pub title: String,
    pub number: u64,
}

/// A GitHub user (login only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
}

/// A pull request review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrReview {
    pub id: u64,
    pub state: String,
    pub body: Option<String>,
    pub user: GitHubUser,
    pub submitted_at: Option<String>,
}

/// An inline review comment on a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrReviewComment {
    pub id: u64,
    pub path: String,
    pub line: Option<u32>,
    pub body: String,
    pub diff_hunk: String,
    pub user: GitHubUser,
    pub created_at: String,
}

/// A pull request ref (head or base).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrRef {
    #[serde(rename = "ref")]
    pub ref_field: String,
    pub sha: String,
}

/// A GitHub pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub head: PrRef,
    pub base: PrRef,
    pub html_url: String,
    pub user: GitHubUser,
    pub body: Option<String>,
    #[serde(default)]
    pub requested_reviewers: Vec<GitHubUser>,
}

/// Fix context: structured review feedback for the PR fix workflow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FixContext {
    pub pr_number: u64,
    pub branch: String,
    pub base_branch: String,
    pub html_url: String,
    pub review_summary: String,
    pub file_comments: Vec<FileCommentGroup>,
}

/// A group of inline comments on the same file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileCommentGroup {
    pub file_path: String,
    pub comments: Vec<InlineComment>,
}

/// A single inline comment extracted from PR review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InlineComment {
    pub line_number: Option<u32>,
    pub diff_hunk: String,
    pub reviewer_comment: String,
    pub reviewer: String,
}

/// Raw issue from GitHub API — includes `pull_request` field for filtering.
#[derive(Debug, Deserialize)]
struct RawGitHubIssue {
    number: u64,
    title: String,
    body: Option<String>,
    labels: Vec<GitHubLabel>,
    milestone: Option<GitHubMilestone>,
    html_url: String,
    state: String,
    pull_request: Option<serde_json::Value>,
}

impl RawGitHubIssue {
    /// Convert to `GitHubIssue`, returning `None` if this is a pull request.
    fn into_issue(self) -> Option<GitHubIssue> {
        if self.pull_request.is_some() {
            return None;
        }
        Some(GitHubIssue {
            number: self.number,
            title: self.title,
            body: self.body,
            labels: self.labels,
            milestone: self.milestone,
            html_url: self.html_url,
            state: self.state,
        })
    }
}

// ── Client ──────────────────────────────────────────────────────────────

/// GitHub REST API client.
///
/// Authenticates via `GITHUB_TOKEN` env var and detects `owner/repo` from
/// the git remote. Uses `reqwest::blocking::Client` with a 5-second timeout.
pub struct GitHubClient {
    token: String,
    owner: String,
    repo: String,
    api_url: String,
    client: reqwest::blocking::Client,
}

// Manual Debug impl to avoid leaking token value in logs.
impl std::fmt::Debug for GitHubClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubClient")
            .field("owner", &self.owner)
            .field("repo", &self.repo)
            .field("api_url", &self.api_url)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl GitHubClient {
    /// Create a new GitHub client.
    ///
    /// Reads `GITHUB_TOKEN` from the environment, parses `owner/repo` from
    /// `git remote get-url origin`, and optionally uses `GITHUB_API_URL` for
    /// GitHub Enterprise (defaults to `https://api.github.com`).
    pub fn new() -> Result<Self, WitPluginError> {
        let token = std::env::var("GITHUB_TOKEN").map_err(|_| {
            WitPluginError::invalid_input("GITHUB_TOKEN environment variable not set")
        })?;

        let api_url = std::env::var("GITHUB_API_URL")
            .unwrap_or_else(|_| "https://api.github.com".to_string());

        let (owner, repo) = parse_owner_repo_from_git()?;

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .user_agent("pulse-auto-dev")
            .build()
            .map_err(|e| github_err(format!("failed to build HTTP client: {e}")))?;

        tracing::info!(
            plugin = "coding-pack",
            owner = %owner,
            repo = %repo,
            "Initializing GitHub client for {}/{}",
            owner,
            repo
        );

        Ok(Self {
            token,
            owner,
            repo,
            api_url,
            client,
        })
    }

    /// Accessor for the repository owner.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Accessor for the repository name.
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// List issues from the repository, filtering out pull requests.
    ///
    /// Handles pagination via the `Link` header, fetching up to 10 pages
    /// (1000 issues max).
    pub fn list_issues(
        &self,
        state: Option<&str>,
        labels: Option<&str>,
        milestone: Option<&str>,
    ) -> Result<Vec<GitHubIssue>, WitPluginError> {
        let base_url = format!(
            "{}/repos/{}/{}/issues",
            self.api_url, self.owner, self.repo
        );

        let mut all_issues: Vec<GitHubIssue> = Vec::new();
        let mut url = base_url.clone();

        // Build initial query params
        let mut params: Vec<(&str, &str)> = vec![("per_page", "100")];
        if let Some(s) = state {
            params.push(("state", s));
        }
        if let Some(l) = labels {
            params.push(("labels", l));
        }
        if let Some(m) = milestone {
            params.push(("milestone", m));
        }

        let max_pages = 10;

        for page_num in 1..=max_pages {
            tracing::debug!(
                plugin = "coding-pack",
                page = page_num,
                "Fetching issues page"
            );

            let resp = self
                .client
                .get(&url)
                .bearer_auth(&self.token)
                .query(&params)
                .send()
                .map_err(|e| github_err(format!("GET {url}: {e}")))?;

            // Check rate limiting
            if let Some(remaining) = resp
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
            {
                if remaining < 100 {
                    tracing::warn!(
                        plugin = "coding-pack",
                        remaining = remaining,
                        "GitHub API rate limit low"
                    );
                }
            }

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().unwrap_or_default();
                return Err(github_err(format!(
                    "GET {url}: HTTP {status} — {body}"
                )));
            }

            // Parse Link header for next page before consuming body
            let next_url = resp
                .headers()
                .get("link")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_link_header_next);

            let body = resp
                .text()
                .map_err(|e| github_err(format!("reading response body: {e}")))?;

            let raw_issues: Vec<RawGitHubIssue> = serde_json::from_str(&body)
                .map_err(|e| github_err(format!("parse issues: {e}")))?;

            let page_count = raw_issues.len();
            let issues: Vec<GitHubIssue> = raw_issues
                .into_iter()
                .filter_map(|r| r.into_issue())
                .collect();

            tracing::debug!(
                plugin = "coding-pack",
                page = page_num,
                raw_count = page_count,
                issue_count = issues.len(),
                "Fetched issues page"
            );

            all_issues.extend(issues);

            // Follow pagination or stop
            match next_url {
                Some(next) => {
                    url = next;
                    // Clear params for subsequent pages — the next URL has them embedded
                    params.clear();
                }
                None => break,
            }
        }

        tracing::info!(
            plugin = "coding-pack",
            total = all_issues.len(),
            "Fetched all issues"
        );

        Ok(all_issues)
    }

    /// List reviews for a pull request, handling pagination.
    pub fn list_pr_reviews(&self, pr_number: u64) -> Result<Vec<PrReview>, WitPluginError> {
        let base_url = format!(
            "{}/repos/{}/{}/pulls/{}/reviews",
            self.api_url, self.owner, self.repo, pr_number
        );

        let mut all_reviews: Vec<PrReview> = Vec::new();
        let mut current_url = Some(base_url);
        let mut is_first = true;

        while let Some(url) = current_url {
            let mut req = self.client.get(&url).bearer_auth(&self.token);
            if is_first {
                req = req.query(&[("per_page", "100")]);
                is_first = false;
            }

            let resp = req
                .send()
                .map_err(|e| github_err(format!("GET {url}: {e}")))?;

            let status = resp.status();
            let link_header = resp
                .headers()
                .get("link")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_link_header_next);

            if !status.is_success() {
                let body = resp.text().unwrap_or_default();
                return Err(github_err(format!(
                    "GET {url}: HTTP {status} — {body}"
                )));
            }

            let body = resp
                .text()
                .map_err(|e| github_err(format!("reading response body: {e}")))?;

            let page: Vec<PrReview> = serde_json::from_str(&body)
                .map_err(|e| github_err(format!("parse reviews: {e}")))?;

            tracing::debug!(
                plugin = "coding-pack",
                pr_number = pr_number,
                page_count = page.len(),
                "Fetched PR reviews page"
            );

            all_reviews.extend(page);
            current_url = link_header;
        }

        Ok(all_reviews)
    }

    /// Get inline review comments for a pull request, handling pagination.
    pub fn get_review_comments(
        &self,
        pr_number: u64,
    ) -> Result<Vec<PrReviewComment>, WitPluginError> {
        let base_url = format!(
            "{}/repos/{}/{}/pulls/{}/comments",
            self.api_url, self.owner, self.repo, pr_number
        );

        let mut all_comments: Vec<PrReviewComment> = Vec::new();
        let mut current_url = Some(base_url);
        let mut is_first = true;

        while let Some(url) = current_url {
            let mut req = self.client.get(&url).bearer_auth(&self.token);
            if is_first {
                req = req.query(&[("per_page", "100")]);
                is_first = false;
            }

            let resp = req
                .send()
                .map_err(|e| github_err(format!("GET {url}: {e}")))?;

            let status = resp.status();
            let link_header = resp
                .headers()
                .get("link")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_link_header_next);

            if !status.is_success() {
                let body = resp.text().unwrap_or_default();
                return Err(github_err(format!(
                    "GET {url}: HTTP {status} — {body}"
                )));
            }

            let body = resp
                .text()
                .map_err(|e| github_err(format!("reading response body: {e}")))?;

            let page: Vec<PrReviewComment> = serde_json::from_str(&body)
                .map_err(|e| github_err(format!("parse review comments: {e}")))?;

            tracing::debug!(
                plugin = "coding-pack",
                pr_number = pr_number,
                page_count = page.len(),
                "Fetched PR review comments page"
            );

            all_comments.extend(page);
            current_url = link_header;
        }

        Ok(all_comments)
    }

    /// List all open pull requests, handling pagination.
    pub fn list_open_prs(&self) -> Result<Vec<PullRequest>, WitPluginError> {
        let base_url = format!(
            "{}/repos/{}/{}/pulls",
            self.api_url, self.owner, self.repo
        );

        let mut all_prs: Vec<PullRequest> = Vec::new();
        let mut current_url = Some(base_url);
        let mut is_first = true;

        while let Some(url) = current_url {
            let mut req = self.client.get(&url).bearer_auth(&self.token);
            if is_first {
                req = req.query(&[("state", "open"), ("per_page", "100")]);
                is_first = false;
            }

            let resp = req
                .send()
                .map_err(|e| github_err(format!("GET {url}: {e}")))?;

            let status = resp.status();
            let link_header = resp
                .headers()
                .get("link")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_link_header_next);

            if !status.is_success() {
                let body = resp.text().unwrap_or_default();
                return Err(github_err(format!(
                    "GET {url}: HTTP {status} — {body}"
                )));
            }

            let body = resp
                .text()
                .map_err(|e| github_err(format!("reading response body: {e}")))?;

            let page: Vec<PullRequest> = serde_json::from_str(&body)
                .map_err(|e| github_err(format!("parse pull requests: {e}")))?;

            tracing::debug!(
                plugin = "coding-pack",
                page_count = page.len(),
                "Fetched open PRs page"
            );

            all_prs.extend(page);
            current_url = link_header;
        }

        tracing::debug!(
            plugin = "coding-pack",
            total = all_prs.len(),
            "Fetched all open PRs"
        );

        Ok(all_prs)
    }

    /// Get a single pull request by number.
    pub fn get_pull_request(&self, pr_number: u64) -> Result<PullRequest, WitPluginError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.api_url, self.owner, self.repo, pr_number
        );

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .map_err(|e| github_err(format!("GET {url}: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(github_err(format!(
                "GET {url}: HTTP {status} — {body}"
            )));
        }

        let body = resp
            .text()
            .map_err(|e| github_err(format!("reading response body: {e}")))?;

        let pr: PullRequest = serde_json::from_str(&body)
            .map_err(|e| github_err(format!("parse pull request: {e}")))?;

        Ok(pr)
    }

    /// Build structured fix context from PR reviews and inline comments.
    ///
    /// Fetches PR details, reviews, and inline comments, then assembles a
    /// `FixContext` suitable for injection into the PR fix workflow.
    pub fn build_fix_context(&self, pr_number: u64) -> Result<FixContext, WitPluginError> {
        let pr = self.get_pull_request(pr_number)?;
        let reviews = self.list_pr_reviews(pr_number)?;
        let comments = self.get_review_comments(pr_number)?;

        // Build review_summary from CHANGES_REQUESTED reviews
        let review_summary = reviews
            .iter()
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
            by_file
                .entry(c.path.clone())
                .or_default()
                .push(InlineComment {
                    line_number: c.line,
                    diff_hunk: c.diff_hunk.clone(),
                    reviewer_comment: c.body.clone(),
                    reviewer: c.user.login.clone(),
                });
        }

        // Sort comments within each file by line number (None sorts first)
        let file_comments: Vec<FileCommentGroup> = by_file
            .into_iter()
            .map(|(file_path, mut comments)| {
                comments.sort_by_key(|c| c.line_number.unwrap_or(0));
                FileCommentGroup {
                    file_path,
                    comments,
                }
            })
            .collect();

        Ok(FixContext {
            pr_number,
            branch: pr.head.ref_field,
            base_branch: pr.base.ref_field,
            html_url: pr.html_url,
            review_summary,
            file_comments,
        })
    }

    /// Request reviewers for a pull request.
    ///
    /// Sends `POST /repos/{owner}/{repo}/pulls/{pr_number}/requested_reviewers`
    /// with the given reviewer logins. No-op if the list is empty.
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
            self.api_url, self.owner, self.repo, pr_number
        );

        let body = serde_json::json!({ "reviewers": reviewers });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .map_err(|e| github_err(format!("POST {url}: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let resp_body = resp.text().unwrap_or_default();
            return Err(github_err(format!(
                "request_reviewers: HTTP {status} for PR #{pr_number}: {resp_body}"
            )));
        }

        tracing::info!(
            plugin = "coding-pack",
            pr_number = pr_number,
            count = reviewers.len(),
            "Re-requested review for PR"
        );

        Ok(())
    }
}

// ── Review state aggregation ─────────────────────────────────────────────

/// Aggregate the review state for a PR from its list of reviews.
///
/// Groups by user, keeps only the latest APPROVED/CHANGES_REQUESTED per user.
/// Returns `"changes_requested"`, `"approved"`, or `"pending"`.
pub fn aggregate_review_state(reviews: &[PrReview]) -> &'static str {
    let mut latest_by_user: std::collections::BTreeMap<&str, &str> =
        std::collections::BTreeMap::new();
    for review in reviews {
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

/// Check if a pull request was created by the auto-dev system.
///
/// Matches by branch prefix `auto-dev/` or body containing
/// `Co-authored-by: pulse-auto-dev`.
pub fn is_auto_dev_pr(pr: &PullRequest) -> bool {
    let branch_match = pr.head.ref_field.starts_with("auto-dev/");
    let body_match = pr
        .body
        .as_deref()
        .map(|b| b.contains("Co-authored-by: pulse-auto-dev"))
        .unwrap_or(false);
    branch_match || body_match
}

// ── Git remote parsing ──────────────────────────────────────────────────

/// Run `git remote get-url origin` and parse owner/repo from the result.
fn parse_owner_repo_from_git() -> Result<(String, String), WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|e| github_err(format!("failed to run git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(github_err(format!("git remote get-url origin failed: {stderr}")));
    }

    let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_remote_url(&remote_url)
}

/// Parse `owner/repo` from a GitHub remote URL.
///
/// Supports:
/// - SSH: `git@github.com:owner/repo.git`
/// - HTTPS: `https://github.com/owner/repo.git`
/// - HTTPS without `.git`: `https://github.com/owner/repo`
fn parse_remote_url(url: &str) -> Result<(String, String), WitPluginError> {
    let url = url.trim();

    // Strip trailing .git
    let url = url.strip_suffix(".git").unwrap_or(url);

    // SSH format: git@github.com:owner/repo
    if let Some(path) = url.strip_prefix("git@") {
        // After git@host:owner/repo
        if let Some(colon_pos) = path.find(':') {
            let path_part = &path[colon_pos + 1..];
            return split_owner_repo(path_part);
        }
    }

    // HTTPS format: https://github.com/owner/repo
    // Also handles http://
    if url.starts_with("https://") || url.starts_with("http://") {
        // Split on '/' and take the last two segments
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 2 {
            let repo = parts[parts.len() - 1];
            let owner = parts[parts.len() - 2];
            if !owner.is_empty() && !repo.is_empty() {
                return Ok((owner.to_string(), repo.to_string()));
            }
        }
    }

    Err(github_err(format!(
        "cannot parse owner/repo from remote URL: {url}"
    )))
}

/// Split "owner/repo" into (owner, repo).
fn split_owner_repo(path: &str) -> Result<(String, String), WitPluginError> {
    let parts: Vec<&str> = path.splitn(2, '/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Ok((parts[0].to_string(), parts[1].to_string()))
    } else {
        Err(github_err(format!(
            "cannot parse owner/repo from path: {path}"
        )))
    }
}

// ── Link header parsing ─────────────────────────────────────────────────

/// Parse the `Link` header value and extract the URL for `rel="next"`.
///
/// GitHub format: `<url>; rel="next", <url>; rel="last"`
fn parse_link_header_next(link: &str) -> Option<String> {
    for segment in link.split(',') {
        let segment = segment.trim();
        if segment.contains("rel=\"next\"") {
            // Extract URL between < and >
            let start = segment.find('<')?;
            let end = segment.find('>')?;
            if start < end {
                return Some(segment[start + 1..end].to_string());
            }
        }
    }
    None
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_remote_https() {
        let (owner, repo) = parse_remote_url("https://github.com/myorg/myrepo.git").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_parse_github_remote_https_no_suffix() {
        let (owner, repo) = parse_remote_url("https://github.com/myorg/myrepo").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_parse_github_remote_ssh() {
        let (owner, repo) = parse_remote_url("git@github.com:myorg/myrepo.git").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_parse_github_remote_ssh_no_suffix() {
        let (owner, repo) = parse_remote_url("git@github.com:myorg/myrepo").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_new_without_token_returns_error() {
        // Temporarily remove GITHUB_TOKEN if it exists
        let original = std::env::var("GITHUB_TOKEN").ok();
        std::env::remove_var("GITHUB_TOKEN");

        let result = GitHubClient::new();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "invalid_input");
        assert!(
            err.message.contains("GITHUB_TOKEN"),
            "Error message should mention GITHUB_TOKEN: {}",
            err.message
        );

        // Restore
        if let Some(val) = original {
            std::env::set_var("GITHUB_TOKEN", val);
        }
    }

    #[test]
    fn test_parse_link_header_next() {
        let link = r#"<https://api.github.com/repos/owner/repo/issues?page=2&per_page=100>; rel="next", <https://api.github.com/repos/owner/repo/issues?page=10&per_page=100>; rel="last""#;
        let next = parse_link_header_next(link);
        assert_eq!(
            next,
            Some("https://api.github.com/repos/owner/repo/issues?page=2&per_page=100".to_string())
        );
    }

    #[test]
    fn test_parse_link_header_none() {
        // Only rel="prev" — no next page
        let link = r#"<https://api.github.com/repos/owner/repo/issues?page=1&per_page=100>; rel="prev""#;
        let next = parse_link_header_next(link);
        assert!(next.is_none());
    }

    #[test]
    fn test_parse_link_header_empty() {
        let next = parse_link_header_next("");
        assert!(next.is_none());
    }

    #[test]
    fn test_filter_pull_requests_from_issues() {
        // Simulates raw JSON that GitHub would return — issues and PRs mixed
        let json = r#"[
            {
                "number": 1,
                "title": "Bug fix",
                "body": "Fix the bug",
                "labels": [{"name": "bug"}],
                "milestone": null,
                "html_url": "https://github.com/o/r/issues/1",
                "state": "open"
            },
            {
                "number": 2,
                "title": "PR for feature",
                "body": "Adds feature",
                "labels": [],
                "milestone": null,
                "html_url": "https://github.com/o/r/pull/2",
                "state": "open",
                "pull_request": {
                    "url": "https://api.github.com/repos/o/r/pulls/2"
                }
            },
            {
                "number": 3,
                "title": "Enhancement",
                "body": null,
                "labels": [{"name": "enhancement"}, {"name": "v2"}],
                "milestone": {"title": "Sprint 5", "number": 5},
                "html_url": "https://github.com/o/r/issues/3",
                "state": "open"
            }
        ]"#;

        let raw_issues: Vec<RawGitHubIssue> = serde_json::from_str(json).unwrap();
        assert_eq!(raw_issues.len(), 3);

        let issues: Vec<GitHubIssue> = raw_issues
            .into_iter()
            .filter_map(|r| r.into_issue())
            .collect();

        // PR (number 2) should be filtered out
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].number, 1);
        assert_eq!(issues[0].title, "Bug fix");
        assert_eq!(issues[0].labels.len(), 1);
        assert_eq!(issues[0].labels[0].name, "bug");

        assert_eq!(issues[1].number, 3);
        assert_eq!(issues[1].title, "Enhancement");
        assert!(issues[1].milestone.is_some());
        assert_eq!(issues[1].milestone.as_ref().unwrap().title, "Sprint 5");
        assert_eq!(issues[1].labels.len(), 2);
    }

    #[test]
    fn test_github_err_helper() {
        let err = github_err("test error");
        assert_eq!(err.code, "internal");
        assert!(err.message.contains("GitHub API error: test error"));
    }

    #[test]
    fn test_parse_remote_url_invalid() {
        let result = parse_remote_url("not-a-url");
        assert!(result.is_err());
    }

    // ── PR review types (Story 23.1) ────────────────────────────────

    #[test]
    fn test_pr_review_deserialization() {
        let json = r#"{
            "id": 100,
            "state": "CHANGES_REQUESTED",
            "body": "Please fix the error handling",
            "user": {"login": "alice"},
            "submitted_at": "2026-03-01T12:00:00Z"
        }"#;
        let review: PrReview = serde_json::from_str(json).unwrap();
        assert_eq!(review.id, 100);
        assert_eq!(review.state, "CHANGES_REQUESTED");
        assert_eq!(review.body.as_deref(), Some("Please fix the error handling"));
        assert_eq!(review.user.login, "alice");
        assert_eq!(
            review.submitted_at.as_deref(),
            Some("2026-03-01T12:00:00Z")
        );
    }

    #[test]
    fn test_pr_review_comment_deserialization() {
        let json = r#"{
            "id": 200,
            "path": "src/main.rs",
            "line": 42,
            "body": "This should handle timeouts",
            "diff_hunk": "@@ -40,6 +40,10 @@ impl Client {\n     pub fn new() ...",
            "user": {"login": "bob"},
            "created_at": "2026-03-01T14:00:00Z"
        }"#;
        let comment: PrReviewComment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, 200);
        assert_eq!(comment.path, "src/main.rs");
        assert_eq!(comment.line, Some(42));
        assert_eq!(comment.body, "This should handle timeouts");
        assert_eq!(comment.user.login, "bob");
    }

    #[test]
    fn test_pr_review_comment_null_line() {
        let json = r#"{
            "id": 201,
            "path": "src/lib.rs",
            "line": null,
            "body": "General comment",
            "diff_hunk": "@@ -0,0 +1,5 @@",
            "user": {"login": "alice"},
            "created_at": "2026-03-01T14:00:00Z"
        }"#;
        let comment: PrReviewComment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.line, None);
    }

    #[test]
    fn test_pull_request_deserialization() {
        let json = r#"{
            "number": 42,
            "title": "Add feature X",
            "head": {"ref": "auto-dev/story-23-1", "sha": "abc123"},
            "base": {"ref": "main", "sha": "def456"},
            "html_url": "https://github.com/o/r/pull/42",
            "user": {"login": "bot"},
            "body": "PR body text",
            "requested_reviewers": [{"login": "alice"}, {"login": "bob"}]
        }"#;
        let pr: PullRequest = serde_json::from_str(json).unwrap();
        assert_eq!(pr.number, 42);
        assert_eq!(pr.title, "Add feature X");
        assert_eq!(pr.head.ref_field, "auto-dev/story-23-1");
        assert_eq!(pr.head.sha, "abc123");
        assert_eq!(pr.base.ref_field, "main");
        assert_eq!(pr.html_url, "https://github.com/o/r/pull/42");
        assert_eq!(pr.user.login, "bot");
        assert_eq!(pr.body.as_deref(), Some("PR body text"));
        assert_eq!(pr.requested_reviewers.len(), 2);
        assert_eq!(pr.requested_reviewers[0].login, "alice");
    }

    #[test]
    fn test_pull_request_no_body_no_reviewers() {
        let json = r#"{
            "number": 1,
            "title": "Simple PR",
            "head": {"ref": "feature/x", "sha": "aaa"},
            "base": {"ref": "main", "sha": "bbb"},
            "html_url": "https://github.com/o/r/pull/1",
            "user": {"login": "dev"}
        }"#;
        let pr: PullRequest = serde_json::from_str(json).unwrap();
        assert!(pr.body.is_none());
        assert!(pr.requested_reviewers.is_empty());
    }

    // ── aggregate_review_state (Story 23.1) ──────────────────────────

    fn make_review(user: &str, state: &str) -> PrReview {
        PrReview {
            id: 0,
            state: state.to_string(),
            body: None,
            user: GitHubUser {
                login: user.to_string(),
            },
            submitted_at: None,
        }
    }

    #[test]
    fn test_aggregate_review_state_changes_requested() {
        // alice approved first, then requested changes
        let reviews = vec![
            make_review("alice", "APPROVED"),
            make_review("alice", "CHANGES_REQUESTED"),
        ];
        assert_eq!(aggregate_review_state(&reviews), "changes_requested");
    }

    #[test]
    fn test_aggregate_review_state_approved() {
        let reviews = vec![
            make_review("alice", "CHANGES_REQUESTED"),
            make_review("alice", "APPROVED"),
        ];
        assert_eq!(aggregate_review_state(&reviews), "approved");
    }

    #[test]
    fn test_aggregate_review_state_pending() {
        // No reviews -> pending
        let reviews: Vec<PrReview> = vec![];
        assert_eq!(aggregate_review_state(&reviews), "pending");
    }

    #[test]
    fn test_aggregate_review_state_only_commented() {
        // Only COMMENTED reviews -> pending (no APPROVED/CHANGES_REQUESTED)
        let reviews = vec![make_review("alice", "COMMENTED")];
        assert_eq!(aggregate_review_state(&reviews), "pending");
    }

    #[test]
    fn test_aggregate_review_state_mixed_users() {
        // alice approved, bob requested changes
        let reviews = vec![
            make_review("alice", "APPROVED"),
            make_review("bob", "CHANGES_REQUESTED"),
        ];
        assert_eq!(aggregate_review_state(&reviews), "changes_requested");
    }

    // ── is_auto_dev_pr (Story 23.1) ──────────────────────────────────

    fn make_pr(branch: &str, body: Option<&str>) -> PullRequest {
        PullRequest {
            number: 1,
            title: "Test PR".to_string(),
            head: PrRef {
                ref_field: branch.to_string(),
                sha: "abc".to_string(),
            },
            base: PrRef {
                ref_field: "main".to_string(),
                sha: "def".to_string(),
            },
            html_url: "https://github.com/o/r/pull/1".to_string(),
            user: GitHubUser {
                login: "bot".to_string(),
            },
            body: body.map(String::from),
            requested_reviewers: vec![],
        }
    }

    #[test]
    fn test_filter_auto_dev_prs_by_branch() {
        let pr = make_pr("auto-dev/story-23-1", None);
        assert!(is_auto_dev_pr(&pr));
    }

    #[test]
    fn test_filter_auto_dev_prs_by_body() {
        let pr = make_pr(
            "feature/custom-branch",
            Some("Some text\nCo-authored-by: pulse-auto-dev\nMore text"),
        );
        assert!(is_auto_dev_pr(&pr));
    }

    #[test]
    fn test_filter_non_auto_dev_prs() {
        let pr = make_pr("feature/manual-work", Some("Manual PR body"));
        assert!(!is_auto_dev_pr(&pr));
    }

    #[test]
    fn test_filter_auto_dev_pr_no_body() {
        let pr = make_pr("feature/x", None);
        assert!(!is_auto_dev_pr(&pr));
    }

    // ── FixContext types (Story 23.2) ────────────────────────────────

    #[test]
    fn test_fix_context_serialization_roundtrip() {
        let ctx = FixContext {
            pr_number: 42,
            branch: "auto-dev/story-23-2".to_string(),
            base_branch: "main".to_string(),
            html_url: "https://github.com/o/r/pull/42".to_string(),
            review_summary: "Reviewer: alice\nFix error handling".to_string(),
            file_comments: vec![FileCommentGroup {
                file_path: "src/lib.rs".to_string(),
                comments: vec![InlineComment {
                    line_number: Some(10),
                    diff_hunk: "@@ -8,3 +8,5 @@".to_string(),
                    reviewer_comment: "Handle timeout".to_string(),
                    reviewer: "alice".to_string(),
                }],
            }],
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: FixContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, deserialized);
    }

    #[test]
    fn test_review_summary_formatting() {
        let reviews = vec![
            PrReview {
                id: 1,
                state: "CHANGES_REQUESTED".to_string(),
                body: Some("Fix error handling".to_string()),
                user: GitHubUser {
                    login: "alice".to_string(),
                },
                submitted_at: None,
            },
            PrReview {
                id: 2,
                state: "APPROVED".to_string(),
                body: Some("Looks good".to_string()),
                user: GitHubUser {
                    login: "carol".to_string(),
                },
                submitted_at: None,
            },
            PrReview {
                id: 3,
                state: "CHANGES_REQUESTED".to_string(),
                body: Some("Add tests".to_string()),
                user: GitHubUser {
                    login: "bob".to_string(),
                },
                submitted_at: None,
            },
        ];

        let summary: String = reviews
            .iter()
            .filter(|r| r.state == "CHANGES_REQUESTED")
            .filter_map(|r| {
                r.body.as_ref().filter(|b| !b.is_empty()).map(|body| {
                    format!("Reviewer: {}\n{}", r.user.login, body)
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        assert_eq!(
            summary,
            "Reviewer: alice\nFix error handling\n\n---\n\nReviewer: bob\nAdd tests"
        );
    }

    #[test]
    fn test_file_comments_grouped_by_path() {
        let comments = vec![
            PrReviewComment {
                id: 1,
                path: "src/a.rs".to_string(),
                line: Some(10),
                body: "Fix this".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "alice".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
            PrReviewComment {
                id: 2,
                path: "src/b.rs".to_string(),
                line: Some(20),
                body: "Fix that".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "bob".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
            PrReviewComment {
                id: 3,
                path: "src/a.rs".to_string(),
                line: Some(30),
                body: "Also this".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "bob".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
        ];

        let mut by_file: std::collections::BTreeMap<String, Vec<InlineComment>> =
            std::collections::BTreeMap::new();
        for c in &comments {
            by_file
                .entry(c.path.clone())
                .or_default()
                .push(InlineComment {
                    line_number: c.line,
                    diff_hunk: c.diff_hunk.clone(),
                    reviewer_comment: c.body.clone(),
                    reviewer: c.user.login.clone(),
                });
        }

        let file_comments: Vec<FileCommentGroup> = by_file
            .into_iter()
            .map(|(file_path, mut cmts)| {
                cmts.sort_by_key(|c| c.line_number.unwrap_or(0));
                FileCommentGroup {
                    file_path,
                    comments: cmts,
                }
            })
            .collect();

        assert_eq!(file_comments.len(), 2);
        assert_eq!(file_comments[0].file_path, "src/a.rs");
        assert_eq!(file_comments[0].comments.len(), 2);
        assert_eq!(file_comments[1].file_path, "src/b.rs");
        assert_eq!(file_comments[1].comments.len(), 1);
    }

    #[test]
    fn test_file_comments_sorted_by_line() {
        let comments = vec![
            PrReviewComment {
                id: 1,
                path: "src/a.rs".to_string(),
                line: Some(50),
                body: "Later".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "a".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
            PrReviewComment {
                id: 2,
                path: "src/a.rs".to_string(),
                line: Some(10),
                body: "Earlier".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "b".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
        ];

        let mut inlines: Vec<InlineComment> = comments
            .iter()
            .map(|c| InlineComment {
                line_number: c.line,
                diff_hunk: c.diff_hunk.clone(),
                reviewer_comment: c.body.clone(),
                reviewer: c.user.login.clone(),
            })
            .collect();

        inlines.sort_by_key(|c| c.line_number.unwrap_or(0));
        assert_eq!(inlines[0].line_number, Some(10));
        assert_eq!(inlines[1].line_number, Some(50));
    }

    #[test]
    fn test_none_line_sorts_first() {
        let mut inlines = vec![
            InlineComment {
                line_number: Some(10),
                diff_hunk: String::new(),
                reviewer_comment: "has line".to_string(),
                reviewer: "a".to_string(),
            },
            InlineComment {
                line_number: None,
                diff_hunk: String::new(),
                reviewer_comment: "no line".to_string(),
                reviewer: "b".to_string(),
            },
        ];

        inlines.sort_by_key(|c| c.line_number.unwrap_or(0));
        assert_eq!(inlines[0].reviewer_comment, "no line");
        assert_eq!(inlines[1].reviewer_comment, "has line");
    }

    #[test]
    fn test_empty_fix_context_when_no_changes_requested() {
        // All APPROVED reviews -> empty review_summary
        let reviews = vec![
            PrReview {
                id: 1,
                state: "APPROVED".to_string(),
                body: Some("Looks good".to_string()),
                user: GitHubUser {
                    login: "alice".to_string(),
                },
                submitted_at: None,
            },
        ];

        let summary: String = reviews
            .iter()
            .filter(|r| r.state == "CHANGES_REQUESTED")
            .filter_map(|r| {
                r.body.as_ref().filter(|b| !b.is_empty()).map(|body| {
                    format!("Reviewer: {}\n{}", r.user.login, body)
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        assert_eq!(summary, "");
    }

    #[test]
    fn test_multiple_reviewers_merged_by_file() {
        // Two different reviewers comment on the same file
        let comments = vec![
            PrReviewComment {
                id: 1,
                path: "src/lib.rs".to_string(),
                line: Some(10),
                body: "Fix A".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "alice".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
            PrReviewComment {
                id: 2,
                path: "src/lib.rs".to_string(),
                line: Some(20),
                body: "Fix B".to_string(),
                diff_hunk: "@@".to_string(),
                user: GitHubUser {
                    login: "bob".to_string(),
                },
                created_at: "2026-01-01".to_string(),
            },
        ];

        let mut by_file: std::collections::BTreeMap<String, Vec<InlineComment>> =
            std::collections::BTreeMap::new();
        for c in &comments {
            by_file
                .entry(c.path.clone())
                .or_default()
                .push(InlineComment {
                    line_number: c.line,
                    diff_hunk: c.diff_hunk.clone(),
                    reviewer_comment: c.body.clone(),
                    reviewer: c.user.login.clone(),
                });
        }

        let file_comments: Vec<FileCommentGroup> = by_file
            .into_iter()
            .map(|(file_path, comments)| FileCommentGroup {
                file_path,
                comments,
            })
            .collect();

        // Both comments in the same FileCommentGroup
        assert_eq!(file_comments.len(), 1);
        assert_eq!(file_comments[0].comments.len(), 2);
        assert_eq!(file_comments[0].comments[0].reviewer, "alice");
        assert_eq!(file_comments[0].comments[1].reviewer, "bob");
    }
}
