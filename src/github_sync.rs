//! GitHub issue sync — imports GitHub issues as board tasks.
//!
//! Syncs open/closed issues from the GitHub repository into the plugin-board
//! task store, creating, updating, or closing tasks as needed.

use crate::board_client;
use crate::github_client::{GitHubClient, GitHubIssue};
use crate::workspace::{GitHubSyncConfig, WorkspaceConfig};
use pulse_plugin_sdk::error::WitPluginError;
use serde::Serialize;
use std::collections::HashMap;

/// Pre-built index of issue_number -> (task_id, status) for batch lookups.
type IssueTaskIndex = HashMap<u64, (String, String)>;

// ── Result types ────────────────────────────────────────────────────────

/// Summary of a sync operation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncResult {
    pub created: u32,
    pub updated: u32,
    pub skipped: u32,
    pub closed: u32,
}

// ── Metadata construction ───────────────────────────────────────────────

/// Build task metadata JSON from a GitHub issue.
fn build_issue_metadata(issue: &GitHubIssue) -> serde_json::Value {
    let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
    let milestone = issue
        .milestone
        .as_ref()
        .map(|m| m.title.as_str())
        .unwrap_or("");

    serde_json::json!({
        "issue_number": issue.number,
        "issue_url": issue.html_url,
        "labels": labels,
        "milestone": milestone,
        "source": "github-sync",
    })
}

// ── Issue filtering ─────────────────────────────────────────────────────

/// Check whether a GitHub issue matches the workspace sync filter.
///
/// - If `filter_labels` is non-empty, the issue must have ALL listed labels (AND logic).
/// - If `filter_milestone` is set, the issue's milestone title must match.
/// - Both checks use AND logic: issue must pass both filters.
/// - If both filters are empty/None, all issues match.
pub fn matches_issue(config: &GitHubSyncConfig, issue: &GitHubIssue) -> bool {
    // Label filter: issue must have ALL configured labels
    if !config.filter_labels.is_empty() {
        let issue_labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
        for required in &config.filter_labels {
            if !issue_labels.contains(&required.as_str()) {
                return false;
            }
        }
    }

    // Milestone filter: issue must be in the configured milestone
    if let Some(ref required_milestone) = config.filter_milestone {
        match &issue.milestone {
            Some(m) if m.title == *required_milestone => {}
            _ => return false,
        }
    }

    true
}

// ── Core sync logic ─────────────────────────────────────────────────────

/// Sync GitHub issues to the board as tasks.
///
/// - Creates new tasks for open issues not yet on the board.
/// - Updates existing tasks when issue title/body has changed.
/// - Closes board tasks whose corresponding GitHub issue has been closed.
/// - Returns a `SyncResult` with counts of each action taken.
pub fn sync_issues_to_board(config: &WorkspaceConfig) -> Result<SyncResult, WitPluginError> {
    let client = GitHubClient::new()?;
    let mut result = SyncResult::default();
    let sync_filter = &config.github_sync;

    // Build filter params from config to pass to both open and closed issue fetches
    let labels_param: Option<String> = if sync_filter.filter_labels.is_empty() {
        None
    } else {
        Some(sync_filter.filter_labels.join(","))
    };
    let milestone_param: Option<&str> = sync_filter.filter_milestone.as_deref();

    // Fetch open issues
    let open_issues = client.list_issues(Some("open"), labels_param.as_deref(), milestone_param)?;
    tracing::info!(
        plugin = "coding-pack",
        count = open_issues.len(),
        "Fetched open GitHub issues"
    );

    // Fetch closed issues (for closing board tasks) — apply same filters to reduce API traffic
    let closed_issues =
        client.list_issues(Some("closed"), labels_param.as_deref(), milestone_param)?;
    tracing::info!(
        plugin = "coding-pack",
        count = closed_issues.len(),
        "Fetched closed GitHub issues"
    );

    // Pre-load issue_number -> (task_id, status) index in one pass to avoid
    // O(issues * tasks) HTTP calls during sync.
    let issue_index = board_client::build_issue_task_index(None)?;

    // Process open issues
    for issue in &open_issues {
        // Apply filter from config
        if !matches_issue(sync_filter, issue) {
            tracing::debug!(
                plugin = "coding-pack",
                issue_number = issue.number,
                "Skipping issue: does not match sync filter"
            );
            result.skipped += 1;
            continue;
        }

        match sync_open_issue(issue, &issue_index) {
            Ok(action) => match action.as_str() {
                "created" => result.created += 1,
                "updated" => result.updated += 1,
                _ => result.skipped += 1,
            },
            Err(e) => {
                tracing::warn!(
                    plugin = "coding-pack",
                    issue_number = issue.number,
                    error = %e,
                    "Failed to sync open issue"
                );
                result.skipped += 1;
            }
        }
    }

    // Process closed issues — close their board tasks if still open
    for issue in &closed_issues {
        match sync_closed_issue(issue, &issue_index) {
            Ok(closed) => {
                if closed {
                    result.closed += 1;
                }
            }
            Err(e) => {
                tracing::warn!(
                    plugin = "coding-pack",
                    issue_number = issue.number,
                    error = %e,
                    "Failed to sync closed issue"
                );
                result.skipped += 1;
            }
        }
    }

    tracing::info!(
        plugin = "coding-pack",
        created = result.created,
        updated = result.updated,
        skipped = result.skipped,
        closed = result.closed,
        "GitHub issue sync complete"
    );

    Ok(result)
}

/// Sync a single open issue — create or update the corresponding board task.
/// Returns the action taken: "created", "updated", or "skipped".
fn sync_open_issue(
    issue: &GitHubIssue,
    issue_index: &IssueTaskIndex,
) -> Result<String, WitPluginError> {
    let existing = issue_index.get(&issue.number).cloned();
    let metadata = build_issue_metadata(issue);
    let description = issue.body.as_deref().unwrap_or("");

    match existing {
        Some((task_id, _status)) => {
            // Update existing task with current title/body/metadata
            let payload = serde_json::json!({
                "title": issue.title,
                "description": description,
                "issue_number": issue.number,
                "issue_url": issue.html_url,
                "labels": metadata["labels"],
                "milestone": metadata["milestone"],
                "source": "github-sync",
            });
            board_client::update_assignment(&task_id, &payload)?;
            tracing::info!(
                plugin = "coding-pack",
                issue_number = issue.number,
                task_id = %task_id,
                "Updated board task from GitHub issue"
            );
            Ok("updated".to_string())
        }
        None => {
            // Create new task
            let task_id = board_client::create_task(
                &issue.title,
                description,
                "ready-for-dev",
                &metadata,
                None,
            )?;
            tracing::info!(
                plugin = "coding-pack",
                issue_number = issue.number,
                task_id = %task_id,
                "Created board task from GitHub issue"
            );
            Ok("created".to_string())
        }
    }
}

/// Sync a single closed issue — if a board task exists and is not already done, close it.
/// Returns `true` if a task was closed.
fn sync_closed_issue(
    issue: &GitHubIssue,
    issue_index: &IssueTaskIndex,
) -> Result<bool, WitPluginError> {
    let existing = issue_index.get(&issue.number).cloned();

    match existing {
        Some((task_id, status)) if status != "done" => {
            board_client::update_assignment(&task_id, &serde_json::json!({"status": "done"}))?;
            tracing::info!(
                plugin = "coding-pack",
                issue_number = issue.number,
                task_id = %task_id,
                "Closed board task (GitHub issue closed)"
            );
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github_client::{GitHubIssue, GitHubLabel, GitHubMilestone};

    /// Helper to build test issues concisely.
    fn test_issue(number: u64, labels: &[&str], milestone: Option<&str>) -> GitHubIssue {
        GitHubIssue {
            number,
            title: format!("Test issue {number}"),
            body: None,
            labels: labels
                .iter()
                .map(|l| GitHubLabel {
                    name: l.to_string(),
                })
                .collect(),
            milestone: milestone.map(|t| GitHubMilestone {
                title: t.to_string(),
                number: 1,
            }),
            html_url: format!("https://github.com/test/repo/issues/{number}"),
            state: "open".to_string(),
        }
    }

    #[test]
    fn test_sync_result_serializes_correctly() {
        let result = SyncResult {
            created: 3,
            updated: 1,
            skipped: 2,
            closed: 0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["created"], 3);
        assert_eq!(json["updated"], 1);
        assert_eq!(json["skipped"], 2);
        assert_eq!(json["closed"], 0);
    }

    #[test]
    fn test_sync_result_default_counts() {
        let result = SyncResult::default();
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.closed, 0);
    }

    #[test]
    fn test_issue_to_task_metadata_shape() {
        let issue = GitHubIssue {
            number: 42,
            title: "Fix login bug".to_string(),
            body: Some("The login page crashes".to_string()),
            labels: vec![
                GitHubLabel {
                    name: "bug".to_string(),
                },
                GitHubLabel {
                    name: "auto-dev".to_string(),
                },
            ],
            milestone: Some(GitHubMilestone {
                title: "Sprint 5".to_string(),
                number: 5,
            }),
            html_url: "https://github.com/owner/repo/issues/42".to_string(),
            state: "open".to_string(),
        };

        let metadata = build_issue_metadata(&issue);

        assert_eq!(metadata["issue_number"], 42);
        assert_eq!(
            metadata["issue_url"],
            "https://github.com/owner/repo/issues/42"
        );
        assert_eq!(metadata["source"], "github-sync");
        assert_eq!(metadata["milestone"], "Sprint 5");

        let labels = metadata["labels"].as_array().unwrap();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0], "bug");
        assert_eq!(labels[1], "auto-dev");
    }

    #[test]
    fn test_issue_metadata_no_milestone() {
        let issue = GitHubIssue {
            number: 10,
            title: "Something".to_string(),
            body: None,
            labels: vec![],
            milestone: None,
            html_url: "https://github.com/o/r/issues/10".to_string(),
            state: "open".to_string(),
        };

        let metadata = build_issue_metadata(&issue);
        assert_eq!(metadata["milestone"], "");
        assert_eq!(metadata["labels"].as_array().unwrap().len(), 0);
    }

    // ── Filter tests (Story 22-3) ──────────────────────────────────

    #[test]
    fn test_matches_issue_no_filters() {
        let config = GitHubSyncConfig::default();
        let issue = test_issue(1, &["bug"], Some("Sprint 1"));
        assert!(matches_issue(&config, &issue));
    }

    #[test]
    fn test_matches_issue_label_filter_matches() {
        let config = GitHubSyncConfig {
            filter_labels: vec!["auto-dev".to_string()],
            filter_milestone: None,
            ..Default::default()
        };
        let issue = test_issue(1, &["auto-dev", "bug"], None);
        assert!(matches_issue(&config, &issue));
    }

    #[test]
    fn test_matches_issue_label_filter_no_match() {
        let config = GitHubSyncConfig {
            filter_labels: vec!["auto-dev".to_string()],
            filter_milestone: None,
            ..Default::default()
        };
        let issue = test_issue(1, &["bug"], None);
        assert!(!matches_issue(&config, &issue));
    }

    #[test]
    fn test_matches_issue_milestone_filter_matches() {
        let config = GitHubSyncConfig {
            filter_labels: vec![],
            filter_milestone: Some("Sprint 5".to_string()),
            ..Default::default()
        };
        let issue = test_issue(1, &[], Some("Sprint 5"));
        assert!(matches_issue(&config, &issue));
    }

    #[test]
    fn test_matches_issue_milestone_filter_no_match() {
        let config = GitHubSyncConfig {
            filter_labels: vec![],
            filter_milestone: Some("Sprint 5".to_string()),
            ..Default::default()
        };
        // Issue in different milestone
        let issue = test_issue(1, &[], Some("Sprint 4"));
        assert!(!matches_issue(&config, &issue));

        // Issue with no milestone
        let issue_no_ms = test_issue(2, &[], None);
        assert!(!matches_issue(&config, &issue_no_ms));
    }

    #[test]
    fn test_matches_issue_both_filters_and_logic() {
        let config = GitHubSyncConfig {
            filter_labels: vec!["auto-dev".to_string()],
            filter_milestone: Some("Sprint 5".to_string()),
            ..Default::default()
        };

        // Both match -> pass
        let issue_both = test_issue(1, &["auto-dev"], Some("Sprint 5"));
        assert!(matches_issue(&config, &issue_both));

        // Label matches but milestone doesn't -> fail
        let issue_label_only = test_issue(2, &["auto-dev"], Some("Sprint 4"));
        assert!(!matches_issue(&config, &issue_label_only));

        // Milestone matches but label doesn't -> fail
        let issue_ms_only = test_issue(3, &["bug"], Some("Sprint 5"));
        assert!(!matches_issue(&config, &issue_ms_only));

        // Neither matches -> fail
        let issue_neither = test_issue(4, &["bug"], Some("Sprint 4"));
        assert!(!matches_issue(&config, &issue_neither));
    }

    #[test]
    fn test_matches_issue_multiple_labels_all_required() {
        let config = GitHubSyncConfig {
            filter_labels: vec!["auto-dev".to_string(), "ready".to_string()],
            filter_milestone: None,
            ..Default::default()
        };

        // Has both required labels -> pass
        let issue_both = test_issue(1, &["auto-dev", "ready", "bug"], None);
        assert!(matches_issue(&config, &issue_both));

        // Has only one of the required labels -> fail
        let issue_one = test_issue(2, &["auto-dev", "bug"], None);
        assert!(!matches_issue(&config, &issue_one));

        // Has none -> fail
        let issue_none = test_issue(3, &["bug"], None);
        assert!(!matches_issue(&config, &issue_none));
    }
}
