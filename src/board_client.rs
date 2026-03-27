//! Board client — capability-based inter-plugin communication with HTTP fallback.
//!
//! Queries board data via the `board.query` capability when running inside Pulse
//! (server/dev mode), falling back to HTTP plugin discovery in standalone CLI mode.
//! Write operations (update, comment) use the Pulse task API directly.

use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

fn api_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("Board API error: {msg}"))
}

// ── Capability constants ─────────────────────────────────────────────────

const BOARD_QUERY_CAP: &str = "board.query";

// ── HTTP fallback helpers ────────────────────────────────────────────────

fn pulse_api_port() -> String {
    std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string())
}

/// Discover plugin-board via the Pulse plugin registry (HTTP fallback for CLI mode).
/// Caches the result for the process lifetime. Validates version >=0.1.0.
fn discover_board_http() -> Result<&'static str, WitPluginError> {
    static DISCOVERY: OnceLock<Result<String, String>> = OnceLock::new();
    let result = DISCOVERY.get_or_init(|| {
        let port = pulse_api_port();
        let url = format!("http://127.0.0.1:{port}/api/v1/plugins");
        let body = reqwest::blocking::get(&url)
            .map_err(|e| format!("GET {url}: {e}"))?
            .text()
            .map_err(|e| format!("read body: {e}"))?;

        let val: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("parse plugins: {e}"))?;

        // Response: { "data": [ { "name": "...", "version": "..." }, ... ] }
        let plugins = val
            .get("data")
            .and_then(|p| p.as_array())
            .or_else(|| val.as_array())
            .ok_or_else(|| "unexpected plugin list format".to_string())?;

        for plugin in plugins {
            let name = plugin.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name == "plugin-board" {
                if let Some(version) = plugin.get("version").and_then(|v| v.as_str()) {
                    validate_board_version(version)?;
                }
                return Ok(name.to_string());
            }
        }
        Err("plugin-board not found in plugin registry".to_string())
    });

    match result {
        Ok(name) => Ok(name.as_str()),
        Err(msg) => Err(api_err(msg)),
    }
}

/// Simple semver check: version >= 0.1.0.
fn validate_board_version(version: &str) -> Result<(), String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 {
        return Err(format!("invalid plugin-board version: {version}"));
    }
    let major: u32 = parts[0]
        .parse()
        .map_err(|_| format!("invalid version: {version}"))?;
    let minor: u32 = parts[1]
        .parse()
        .map_err(|_| format!("invalid version: {version}"))?;
    if major == 0 && minor < 1 {
        return Err(format!(
            "plugin-board version {version} < 0.1.0 — incompatible"
        ));
    }
    Ok(())
}

fn board_api_http(endpoint: &str) -> Result<String, WitPluginError> {
    let plugin_name = discover_board_http()?;
    let port = pulse_api_port();
    let endpoint = endpoint.trim_start_matches('/');
    Ok(format!(
        "http://127.0.0.1:{port}/api/v1/plugins/{plugin_name}/data/{endpoint}"
    ))
}

// ── Core query function ──────────────────────────────────────────────────

/// Query the board plugin — capability RPC first, HTTP discovery fallback.
fn board_query(
    endpoint: &str,
    workspace: Option<&str>,
) -> Result<serde_json::Value, WitPluginError> {
    // 1. Capability-based RPC (available in server/dev mode)
    if pulse_plugin_sdk::host::is_host_available() {
        let mut params = serde_json::json!({ "endpoint": endpoint });
        if let Some(ws) = workspace {
            params["workspace"] = serde_json::Value::String(ws.to_string());
        }
        return pulse_plugin_sdk::host::call_capability(BOARD_QUERY_CAP, params);
    }

    // 2. HTTP fallback (CLI mode) — discover plugin-board dynamically
    let mut url = board_api_http(endpoint)?;
    if let Some(ws) = workspace {
        url.push_str(&format!("?workspace={ws}"));
    }
    let body = reqwest::blocking::get(&url)
        .map_err(|e| api_err(format!("GET {url}: {e}")))?
        .text()
        .map_err(api_err)?;
    serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))
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

// ── API calls ────────────────────────────────────────────────────────────

/// Check if the board plugin is reachable (via capability or HTTP discovery).
pub fn is_available() -> bool {
    board_query("board/boards/list", None).is_ok()
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
    let resp = board_query("board/data", workspace)?;

    let items = resp
        .get("items")
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();

    let assignments: Vec<Assignment> = items
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .filter(|a: &Assignment| status_filter.is_none_or(|s| a.status == s))
        .collect();
    Ok(assignments)
}

/// Update an assignment's fields (merge semantics) via Pulse task API.
pub fn update_assignment(
    task_id: &str,
    payload: &serde_json::Value,
) -> Result<(), WitPluginError> {
    let port = pulse_api_port();
    let url = format!("http://127.0.0.1:{port}/api/v1/tasks/{task_id}/metadata");
    let client = reqwest::blocking::Client::new();
    client
        .patch(&url)
        .json(payload)
        .send()
        .map_err(|e| api_err(format!("PATCH {url}: {e}")))?;
    Ok(())
}

/// Add a comment to an assignment via Pulse task API.
pub fn add_comment(task_id: &str, content: &str, author: &str) -> Result<(), WitPluginError> {
    let port = pulse_api_port();
    let url = format!("http://127.0.0.1:{port}/api/v1/tasks/{task_id}/metadata");

    // Read existing metadata, append comment
    let task_url = format!("http://127.0.0.1:{port}/api/v1/tasks/{task_id}");
    let body = reqwest::blocking::get(&task_url)
        .map_err(|e| api_err(format!("GET {task_url}: {e}")))?
        .text()
        .map_err(api_err)?;

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

/// Create a new task via Pulse task API.
///
/// When `workspace` is provided it is included in the payload so the task is
/// associated with the correct Pulse workspace.  Pass `None` to use the
/// server default.
///
/// Returns the created task's ID.
pub fn create_task(
    title: &str,
    description: &str,
    status: &str,
    metadata: &serde_json::Value,
    workspace: Option<&str>,
) -> Result<String, WitPluginError> {
    let port = pulse_api_port();
    let url = format!("http://127.0.0.1:{port}/api/v1/tasks");
    let mut payload = serde_json::json!({
        "title": title,
        "description": description,
        "status": status,
        "metadata": metadata,
    });
    if let Some(ws) = workspace {
        payload["workspace_id"] = serde_json::Value::String(ws.to_string());
    }

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .map_err(|e| api_err(format!("POST {url}: {e}")))?;

    let body = resp.text().map_err(api_err)?;
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse create response: {e}")))?;

    // Extract task ID from response — could be in "id", "task.id", or "data.id"
    let task_id = val
        .get("id")
        .or_else(|| val.get("task").and_then(|t| t.get("id")))
        .or_else(|| val.get("data").and_then(|d| d.get("id")))
        .and_then(|v| v.as_str())
        .ok_or_else(|| api_err("create task response missing id field"))?;

    Ok(task_id.to_string())
}

/// Find a board task by its `issue_number` metadata field.
///
/// Scans tasks (optionally scoped to `workspace`) and returns
/// `(task_id, current_status)` for the first match.
///
/// **Note:** board data items do not include metadata, so each task's Pulse
/// record is fetched individually.  Pass a `workspace` to narrow the scan.
pub fn find_task_by_issue_number(
    issue_number: u64,
    workspace: Option<&str>,
) -> Result<Option<(String, String)>, WitPluginError> {
    let assignments = list_assignments_in_workspace(None, workspace)?;

    let port = pulse_api_port();

    for assignment in &assignments {
        let task_url = format!("http://127.0.0.1:{port}/api/v1/tasks/{}", assignment.id);
        let body = match reqwest::blocking::get(&task_url) {
            Ok(resp) => match resp.text() {
                Ok(b) => b,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let val: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Check metadata.issue_number — could be at root or under "task"
        let task_obj = val.get("task").unwrap_or(&val);
        if let Some(meta) = task_obj.get("metadata") {
            if let Some(num) = meta.get("issue_number").and_then(|n| n.as_u64()) {
                if num == issue_number {
                    return Ok(Some((
                        assignment.id.clone(),
                        assignment.status.clone(),
                    )));
                }
            }
        }
    }

    Ok(None)
}

/// Get task metadata from the Pulse task API.
///
/// Returns the metadata JSON object, or an empty object if no metadata.
pub fn get_task_metadata(task_id: &str) -> Result<serde_json::Value, WitPluginError> {
    let port = pulse_api_port();
    let task_url = format!("http://127.0.0.1:{port}/api/v1/tasks/{task_id}");
    let body = reqwest::blocking::get(&task_url)
        .map_err(|e| api_err(format!("GET {task_url}: {e}")))?
        .text()
        .map_err(api_err)?;

    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;

    let task_obj = val.get("task").unwrap_or(&val);
    Ok(task_obj
        .get("metadata")
        .cloned()
        .unwrap_or(serde_json::json!({})))
}

// ── Unit tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_board_version_accepts_valid() {
        assert!(validate_board_version("0.1.0").is_ok());
        assert!(validate_board_version("0.2.0").is_ok());
        assert!(validate_board_version("1.0.0").is_ok());
        assert!(validate_board_version("2.3.4").is_ok());
    }

    #[test]
    fn validate_board_version_rejects_too_old() {
        assert!(validate_board_version("0.0.1").is_err());
        assert!(validate_board_version("0.0.0").is_err());
    }

    #[test]
    fn validate_board_version_rejects_invalid() {
        assert!(validate_board_version("abc").is_err());
        assert!(validate_board_version("").is_err());
    }

    #[test]
    fn assignment_deserializes_with_nulls() {
        let json = serde_json::json!({
            "id": "task-1",
            "title": null,
            "status": "ready-for-dev",
            "description": null,
            "priority": "high",
            "assignee": null,
            "labels": ["story"],
            "workflow_id": null,
        });
        let a: Assignment = serde_json::from_value(json).unwrap();
        assert_eq!(a.id, "task-1");
        assert_eq!(a.title, "");
        assert_eq!(a.status, "ready-for-dev");
        assert_eq!(a.priority, "high");
        assert_eq!(a.labels, vec!["story"]);
    }

    #[test]
    fn assignment_deserializes_minimal() {
        let json = serde_json::json!({ "id": "t1" });
        let a: Assignment = serde_json::from_value(json).unwrap();
        assert_eq!(a.id, "t1");
        assert_eq!(a.status, "");
    }

}
