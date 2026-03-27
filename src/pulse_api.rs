//! Pulse Task API client — unified data source for the board.
//!
//! All board operations go through the Pulse REST API (`/api/v1/tasks`).
//! Task metadata (title, description, priority, labels, subtasks, comments)
//! is stored in the `metadata` JSON field on Pulse tasks.

use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};

// ── Config ────────────────────────────────────────────────────────────────

fn api_base() -> String {
    let port = std::env::var("PULSE_API_PORT").unwrap_or_else(|_| "8080".to_string());
    format!("http://127.0.0.1:{}/api/v1", port)
}

fn api_err(msg: impl std::fmt::Display) -> WitPluginError {
    WitPluginError::internal(format!("Pulse API error: {msg}"))
}

// ── Pulse task types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseTask {
    pub id: String,
    pub workflow_id: String,
    #[serde(default)]
    pub step_id: String,
    pub state: String,
    pub created_at: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub metadata: Option<TaskMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskMetadata {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub assignee: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub subtasks: Vec<SubTask>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub workflow_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub content: String,
    #[serde(default)]
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
struct TaskListResponse {
    items: Vec<PulseTask>,
}

#[derive(Debug, Deserialize)]
struct TaskDetailResponse {
    #[serde(flatten)]
    task: PulseTask,
}

// ── API calls ─────────────────────────────────────────────────────────────

/// List all tasks from Pulse API.
pub fn list_tasks() -> Result<Vec<PulseTask>, WitPluginError> {
    let url = format!("{}/tasks?limit=500", api_base());
    let body = reqwest::blocking::get(&url)
        .map_err(|e| api_err(format!("GET {url}: {e}")))?
        .text()
        .map_err(|e| api_err(e))?;
    let resp: TaskListResponse =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;
    Ok(resp.items)
}

/// Get a single task by ID.
pub fn get_task(task_id: &str) -> Result<PulseTask, WitPluginError> {
    let url = format!("{}/tasks/{}", api_base(), task_id);
    let body = reqwest::blocking::get(&url)
        .map_err(|e| api_err(format!("GET {url}: {e}")))?
        .text()
        .map_err(|e| api_err(e))?;

    // The detail response wraps task in a "task" field with timeline/events
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;

    // Try nested "task" field first (detail response), then flat
    let task_val = if val.get("task").is_some() {
        val["task"].clone()
    } else {
        val
    };

    serde_json::from_value(task_val).map_err(|e| api_err(format!("deserialize task: {e}")))
}

/// Create a new task with metadata.
pub fn create_task(metadata: &TaskMetadata) -> Result<String, WitPluginError> {
    let url = format!("{}/tasks", api_base());
    let payload = serde_json::json!({
        "workflow_id": if metadata.workflow_id.is_empty() { "board" } else { &metadata.workflow_id },
        "state": match metadata.status.as_str() {
            "ready-for-dev" => "ReQueued",
            "in-progress" => "Running",
            "review" => "AwaitingReview",
            "done" => "Completed",
            _ => "Pending",
        },
        "metadata": metadata,
    });

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .map_err(|e| api_err(format!("POST {url}: {e}")))?;

    let body = resp.text().map_err(|e| api_err(e))?;
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| api_err(format!("parse: {e}")))?;

    val["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| api_err("no id in response"))
}

/// Update task metadata (full replace).
pub fn update_metadata(task_id: &str, metadata: &TaskMetadata) -> Result<(), WitPluginError> {
    let url = format!("{}/tasks/{}/metadata", api_base(), task_id);
    let client = reqwest::blocking::Client::new();
    client
        .patch(&url)
        .json(&serde_json::to_value(metadata).map_err(|e| api_err(e))?)
        .send()
        .map_err(|e| api_err(format!("PATCH {url}: {e}")))?;
    Ok(())
}

// ── High-level board operations ───────────────────────────────────────────

/// Get metadata for a task, returning default if none set.
fn get_meta(task: &PulseTask) -> TaskMetadata {
    task.metadata.clone().unwrap_or_default()
}

/// Board status from Pulse task state.
fn pulse_state_to_board_status(state: &str, meta: &TaskMetadata) -> String {
    // Prefer metadata.status if set
    if !meta.status.is_empty() {
        return meta.status.clone();
    }
    match state {
        "Pending" => "backlog",
        "Running" => "in-progress",
        "Completed" => "done",
        "Failed" => "backlog",
        "ReQueued" | "AwaitingHuman" => "ready-for-dev",
        "HumanReview" | "AwaitingReview" => "review",
        _ => "backlog",
    }
    .to_string()
}

/// Get board data for the Kanban view from Pulse tasks.
pub fn get_board_data() -> Result<serde_json::Value, WitPluginError> {
    let tasks = list_tasks()?;

    let items: Vec<serde_json::Value> = tasks
        .iter()
        .filter(|t| t.metadata.is_some()) // only show tasks with board metadata
        .map(|t| {
            let meta = get_meta(t);
            let status = pulse_state_to_board_status(&t.state, &meta);
            let total = meta.subtasks.len();
            let done = meta.subtasks.iter().filter(|s| s.done).count();
            let pct = if total > 0 {
                Some((done as f64 / total as f64 * 1000.0).round() / 10.0)
            } else {
                None
            };

            serde_json::json!({
                "id": t.id,
                "type": "assignment",
                "title": if meta.title.is_empty() { &t.workflow_id } else { &meta.title },
                "status": status,
                "phase": 0,
                "epic_id": meta.assignee,
                "epic_title": format!("{}/{} tasks", done, total),
                "story_number": meta.priority,
                "story_count": total,
                "stories_done": done,
                "progress_pct": pct,
                "comment_count": meta.comments.len(),
                "labels": meta.labels,
                "description": meta.description,
                "assignee": if meta.assignee.is_empty() { None } else { Some(&meta.assignee) },
            })
        })
        .collect();

    let total = items.len();
    let done = items.iter().filter(|i| i["status"] == "done").count();
    let progress_pct = if total > 0 {
        ((done as f64 / total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    Ok(serde_json::json!({
        "project": "Pulse",
        "last_updated": "",
        "phases": [],
        "summary": {
            "total_epics": 0,
            "total_stories": total,
            "done_epics": 0,
            "done_stories": done,
            "in_progress_stories": items.iter().filter(|i| i["status"] == "in-progress").count(),
            "ready_stories": items.iter().filter(|i| i["status"] == "ready-for-dev").count(),
            "backlog_stories": items.iter().filter(|i| i["status"] == "backlog").count(),
            "review_stories": items.iter().filter(|i| i["status"] == "review").count(),
            "progress_pct": progress_pct,
        },
        "items": items,
    }))
}

/// Get assignment detail for modal display, including LLM execution events.
pub fn get_assignment_detail(task_id: &str) -> Result<serde_json::Value, WitPluginError> {
    let task = get_task(task_id)?;
    let meta = get_meta(&task);
    let total = meta.subtasks.len();
    let done = meta.subtasks.iter().filter(|s| s.done).count();

    // Fetch full task detail with events for LLM records
    let detail_url = format!("{}/tasks/{}", api_base(), task_id);
    let events = reqwest::blocking::get(&detail_url)
        .ok()
        .and_then(|r| r.text().ok())
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|v| v.get("events").cloned())
        .and_then(|e| e.as_array().cloned())
        .unwrap_or_default();

    // Extract LLM execution records from events
    let mut llm_records: Vec<serde_json::Value> = Vec::new();
    for event in &events {
        if let Some(completed) = event.get("Completed") {
            if let Some(output) = completed.get("output") {
                let content = output.get("content").and_then(|c| c.get("content"));
                let step_id = output
                    .get("content")
                    .and_then(|c| c.get("step_id"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let executor = output
                    .get("content")
                    .and_then(|c| c.get("executor"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let status = output
                    .get("content")
                    .and_then(|c| c.get("status"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let exec_time = output
                    .get("content")
                    .and_then(|c| c.get("execution_time_ms"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                // Parse the inner content JSON (LLM response)
                let llm_data = content
                    .and_then(|c| c.as_str())
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

                let result_text = llm_data
                    .as_ref()
                    .and_then(|d| d.get("result"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                let session_id = llm_data
                    .as_ref()
                    .and_then(|d| d.get("session_id"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let cost_usd = llm_data
                    .as_ref()
                    .and_then(|d| d.get("total_cost_usd"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let input_tokens = llm_data
                    .as_ref()
                    .and_then(|d| d.get("input_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let output_tokens = llm_data
                    .as_ref()
                    .and_then(|d| d.get("output_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                let timestamp = completed
                    .get("completed_at")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                llm_records.push(serde_json::json!({
                    "step_id": step_id,
                    "executor": executor,
                    "status": status,
                    "execution_time_ms": exec_time,
                    "result": result_text,
                    "session_id": session_id,
                    "cost_usd": cost_usd,
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                    "timestamp": timestamp,
                }));
            }
        }
    }

    // Merge LLM records into comments (as system-generated entries)
    let mut all_comments: Vec<serde_json::Value> = meta
        .comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "author": c.author,
                "content": c.content,
                "timestamp": c.timestamp,
            })
        })
        .collect();

    for (i, rec) in llm_records.iter().enumerate() {
        let cost_str = if rec["cost_usd"].as_f64().unwrap_or(0.0) > 0.0 {
            format!(
                " | ${:.4} ({} in / {} out tokens)",
                rec["cost_usd"].as_f64().unwrap_or(0.0),
                rec["input_tokens"],
                rec["output_tokens"]
            )
        } else {
            String::new()
        };

        all_comments.push(serde_json::json!({
            "id": format!("llm-{}", i + 1),
            "author": format!("{} ({})", rec["executor"].as_str().unwrap_or("llm"), rec["step_id"].as_str().unwrap_or("")),
            "content": format!("{}\n\n⏱ {}ms{}", rec["result"].as_str().unwrap_or(""), rec["execution_time_ms"], cost_str),
            "timestamp": rec["timestamp"].as_str().unwrap_or(""),
        }));
    }

    Ok(serde_json::json!({
        "id": task.id,
        "title": if meta.title.is_empty() { &task.workflow_id } else { &meta.title },
        "status": pulse_state_to_board_status(&task.state, &meta),
        "description": meta.description,
        "assignee": meta.assignee,
        "priority": meta.priority,
        "labels": meta.labels,
        "workflow_id": meta.workflow_id,
        "task_progress": format!("{}/{}", done, total),
        "task_count": total,
        "tasks_done": done,
        "tasks": meta.subtasks.iter().map(|s| serde_json::json!({
            "id": s.id,
            "title": s.title,
            "done": s.done,
        })).collect::<Vec<_>>(),
        "comments": all_comments,
        "llm_records": llm_records,
        "timeline": events,
    }))
}

/// Create a new board assignment via Pulse API.
pub fn create_assignment(payload: &serde_json::Value) -> Result<serde_json::Value, WitPluginError> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'title' field required"))?;

    let subtasks: Vec<SubTask> = payload
        .get("tasks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    v.as_str().map(|s| SubTask {
                        id: format!("st-{}", i + 1),
                        title: s.to_string(),
                        done: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let meta = TaskMetadata {
        title: title.to_string(),
        description: payload
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        status: payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("backlog")
            .to_string(),
        priority: payload
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_string(),
        assignee: payload
            .get("assignee")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        labels: payload
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        subtasks,
        comments: vec![],
        workflow_id: payload
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    };

    let task_id = create_task(&meta)?;

    Ok(serde_json::json!({
        "id": task_id,
        "title": meta.title,
        "status": meta.status,
        "priority": meta.priority,
    }))
}

/// Update assignment fields via metadata PATCH.
pub fn update_assignment(
    task_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let task = get_task(task_id)?;
    let mut meta = get_meta(&task);

    if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
        meta.title = title.to_string();
    }
    if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
        meta.status = status.to_string();
    }
    if let Some(desc) = payload.get("description").and_then(|v| v.as_str()) {
        meta.description = desc.to_string();
    }
    if let Some(assignee) = payload.get("assignee").and_then(|v| v.as_str()) {
        meta.assignee = assignee.to_string();
    }
    if let Some(priority) = payload.get("priority").and_then(|v| v.as_str()) {
        meta.priority = priority.to_string();
    }

    update_metadata(task_id, &meta)?;

    Ok(serde_json::json!({
        "id": task_id,
        "title": meta.title,
        "status": meta.status,
    }))
}

/// Add a comment to a task.
pub fn add_comment(
    task_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'content' field required"))?;
    let author = payload
        .get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("LLM Agent");

    let task = get_task(task_id)?;
    let mut meta = get_meta(&task);
    let comment_num = meta.comments.len() + 1;
    let comment = Comment {
        id: format!("comment-{}", comment_num),
        author: author.to_string(),
        content: content.to_string(),
        timestamp: today_string(),
    };

    let result = serde_json::to_value(&comment).map_err(|e| api_err(e))?;
    meta.comments.push(comment);
    update_metadata(task_id, &meta)?;
    Ok(result)
}

/// Add a subtask to a task.
pub fn add_subtask(
    task_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'title' field required"))?;

    let task = get_task(task_id)?;
    let mut meta = get_meta(&task);
    let st_num = meta.subtasks.len() + 1;
    let subtask = SubTask {
        id: format!("st-{}", st_num),
        title: title.to_string(),
        done: false,
    };

    let result = serde_json::to_value(&subtask).map_err(|e| api_err(e))?;
    meta.subtasks.push(subtask);
    update_metadata(task_id, &meta)?;
    Ok(result)
}

/// Toggle a subtask's done status.
pub fn toggle_subtask(
    task_id: &str,
    subtask_id: &str,
) -> Result<serde_json::Value, WitPluginError> {
    let task = get_task(task_id)?;
    let mut meta = get_meta(&task);

    let subtask = meta
        .subtasks
        .iter_mut()
        .find(|s| s.id == subtask_id)
        .ok_or_else(|| WitPluginError::not_found(format!("Subtask '{subtask_id}' not found")))?;

    subtask.done = !subtask.done;

    let result = serde_json::json!({
        "id": subtask.id,
        "title": subtask.title,
        "done": subtask.done,
    });

    update_metadata(task_id, &meta)?;
    Ok(result)
}

/// Get filter options.
pub fn get_filter_options() -> Result<serde_json::Value, WitPluginError> {
    Ok(serde_json::json!({
        "phases": [],
        "epics": [],
        "statuses": [
            {"value": "backlog", "label": "Backlog"},
            {"value": "ready-for-dev", "label": "Ready for Dev"},
            {"value": "in-progress", "label": "In Progress"},
            {"value": "review", "label": "Review"},
            {"value": "done", "label": "Done"}
        ],
        "types": [{"value": "assignment", "label": "Assignment"}]
    }))
}

fn today_string() -> String {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    format!("{y}-{m:02}-{d:02}")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
