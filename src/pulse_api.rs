//! Minimal Pulse Task API client.
//!
//! Board-specific operations have moved to plugin-board.
//! This module retains only `get_task()` for workspace resolution in lib.rs.

use pulse_plugin_sdk::error::WitPluginError;
use serde::Deserialize;

fn api_base() -> String {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    format!("http://127.0.0.1:{}/api/v1", port)
}

fn api_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("Pulse API error: {msg}"))
}

#[derive(Debug, Clone, Deserialize)]
pub struct PulseTask {
    pub id: String,
    #[serde(default)]
    pub workflow_id: String,
    #[serde(default)]
    pub state: String,
    #[serde(default, alias = "workspace")]
    pub workspace_id: String,
}

/// Get a single task by ID (used for workspace resolution).
pub fn get_task(task_id: &str) -> Result<PulseTask, WitPluginError> {
    let url = format!("{}/tasks/{}", api_base(), task_id);
    let body = reqwest::blocking::get(&url)
        .map_err(|e| api_err(format!("GET {url}: {e}")))?
        .text()
        .map_err(|e| api_err(e))?;

    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;

    let task_val = if val.get("task").is_some() {
        val["task"].clone()
    } else {
        val
    };

    serde_json::from_value(task_val).map_err(|e| api_err(format!("deserialize task: {e}")))
}
