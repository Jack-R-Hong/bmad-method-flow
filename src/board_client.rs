//! HTTP client for the plugin-board plugin.
//!
//! Replaces direct `board_store` calls with HTTP requests to the board plugin
//! via the Pulse API proxy at `/api/v1/plugins/plugin-board/data/...`.

use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};

fn board_api(path: &str) -> String {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    let path = path.trim_start_matches('/');
    format!("http://127.0.0.1:{}/api/v1/plugins/plugin-board/data/{}", port, path)
}

fn api_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("Board API error: {msg}"))
}

// ── Types ────────────────────────────────────────────────────────────────

fn nullable_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(d).map(|o| o.unwrap_or_default())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assignment {
    pub id: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub title: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub status: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub description: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub priority: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub assignee: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default, deserialize_with = "nullable_string")]
    pub workflow_id: String,
}

#[derive(Debug, Deserialize)]
struct BoardDataResponse {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

// ── API calls ────────────────────────────────────────────────────────────

/// Check if the board plugin is available.
pub fn is_available() -> bool {
    let url = board_api("board/boards/list");
    reqwest::blocking::get(&url).is_ok()
}

/// List assignments from the board plugin, optionally filtered by status.
/// Fetches all tasks across all workspaces (no workspace filter).
pub fn list_assignments(status_filter: Option<&str>) -> Result<Vec<Assignment>, WitPluginError> {
    list_assignments_in_workspace(status_filter, None)
}

/// List assignments from the board plugin for a specific workspace.
pub fn list_assignments_in_workspace(
    status_filter: Option<&str>,
    workspace: Option<&str>,
) -> Result<Vec<Assignment>, WitPluginError> {
    let mut url = board_api("board/data");
    if let Some(ws) = workspace {
        url.push_str(&format!("?workspace={}", ws));
    }
    let body = reqwest::blocking::get(&url)
        .map_err(|e| api_err(format!("GET {url}: {e}")))?
        .text()
        .map_err(|e| api_err(e))?;
    let resp: BoardDataResponse =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;

    let assignments: Vec<Assignment> = resp
        .items
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .filter(|a: &Assignment| {
            status_filter.map_or(true, |s| a.status == s)
        })
        .collect();
    Ok(assignments)
}

/// Update an assignment's fields (merge semantics).
pub fn update_assignment(task_id: &str, payload: &serde_json::Value) -> Result<(), WitPluginError> {
    // Use Pulse task API directly since board plugin routes through it
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}/api/v1/tasks/{}/metadata", port, task_id);
    let client = reqwest::blocking::Client::new();
    client
        .patch(&url)
        .json(payload)
        .send()
        .map_err(|e| api_err(format!("PATCH {url}: {e}")))?;
    Ok(())
}

/// Add a comment to an assignment.
pub fn add_comment(task_id: &str, content: &str, author: &str) -> Result<(), WitPluginError> {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}/api/v1/tasks/{}/metadata", port, task_id);

    // Read existing metadata, append comment
    let task_url = format!("http://127.0.0.1:{}/api/v1/tasks/{}", port, task_id);
    let body = reqwest::blocking::get(&task_url)
        .map_err(|e| api_err(format!("GET {task_url}: {e}")))?
        .text()
        .map_err(|e| api_err(e))?;

    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;
    let task_val = val.get("task").unwrap_or(&val);
    let mut meta = task_val
        .get("metadata")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let comments = meta
        .get("comments")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let num = comments.len() + 1;
    let mut new_comments = comments;
    new_comments.push(serde_json::json!({
        "id": format!("comment-{num}"),
        "author": author,
        "content": content,
        "timestamp": "",
    }));
    meta["comments"] = serde_json::Value::Array(new_comments);

    let client = reqwest::blocking::Client::new();
    client
        .patch(&url)
        .json(&meta)
        .send()
        .map_err(|e| api_err(format!("PATCH {url}: {e}")))?;
    Ok(())
}
