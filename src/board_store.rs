//! JSON file-based board store for dynamic epic/story CRUD.
//!
//! Provides persistence at `_bmad-output/board-store.json` with atomic writes.
//! When the store file exists, board read functions use it instead of parsing
//! YAML/markdown artifacts.

use crate::board;
use crate::workspace::WorkspaceConfig;
use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Constants ────────────────────────────────────────────────────────────────

const STORE_REL_PATH: &str = "_bmad-output/board-store.json";
const VALID_STATUSES: &[&str] = &["backlog", "ready-for-dev", "in-progress", "review", "done"];

// ── Store types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardStore {
    pub version: u32,
    pub project: String,
    pub last_updated: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_from: Option<String>,
    pub epics: Vec<StoreEpic>,
    #[serde(default)]
    pub assignments: Vec<StoreAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreAssignment {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub assignee: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<SubTask>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    /// Explicit workflow ID override for auto-dev. If empty, resolved from labels.
    #[serde(default, skip_serializing_if = "String::is_empty")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEpic {
    pub number: u32,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub frs_covered: String,
    #[serde(default)]
    pub nfrs_covered: String,
    pub stories: Vec<StoreStory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStory {
    pub id: String,
    pub story_number: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub user_story: String,
    #[serde(default)]
    pub acceptance_criteria: String,
}

// ── File operations ──────────────────────────────────────────────────────────

pub fn store_path(base_dir: &Path) -> PathBuf {
    base_dir.join(STORE_REL_PATH)
}

pub fn store_exists(base_dir: &Path) -> bool {
    store_path(base_dir).exists()
}

pub fn load_store(base_dir: &Path) -> Result<BoardStore, WitPluginError> {
    let path = store_path(base_dir);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| WitPluginError::not_found(format!("board-store.json: {e}")))?;
    serde_json::from_str(&content)
        .map_err(|e| WitPluginError::internal(format!("Invalid board-store.json: {e}")))
}

pub fn save_store(base_dir: &Path, store: &BoardStore) -> Result<(), WitPluginError> {
    let path = store_path(base_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WitPluginError::internal(format!("Cannot create directory: {e}")))?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))?;
    std::fs::write(&tmp_path, &json)
        .map_err(|e| WitPluginError::internal(format!("Cannot write temp file: {e}")))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| WitPluginError::internal(format!("Atomic rename failed: {e}")))?;
    Ok(())
}

fn today_string() -> String {
    // Use system time to produce YYYY-MM-DD
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple date calculation (no leap-second precision needed)
    let days = secs / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!("{year}-{month:02}-{day:02}")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    //算法: civil_from_days (Howard Hinnant)
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

fn validate_status(status: &str) -> Result<(), WitPluginError> {
    if VALID_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(WitPluginError::invalid_input(format!(
            "Invalid status '{}'. Valid: {}",
            status,
            VALID_STATUSES.join(", ")
        )))
    }
}

fn title_to_slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── Sync from YAML/markdown ──────────────────────────────────────────────────

pub fn sync_from_artifacts(config: &WorkspaceConfig) -> Result<BoardStore, WitPluginError> {
    let (epics, project, last_updated) = board::parse_sprint_status(&config.base_dir)?;
    let md_metadata = board::parse_epics_markdown(&config.base_dir);

    let store_epics: Vec<StoreEpic> = epics
        .iter()
        .map(|epic| {
            let md = md_metadata.get(&epic.number);
            let stories: Vec<StoreStory> = epic
                .stories
                .iter()
                .map(|s| {
                    let md_story = md.and_then(|m| m.stories.get(&s.story_number));
                    StoreStory {
                        id: s.id.clone(),
                        story_number: s.story_number.clone(),
                        title: s.title.clone(),
                        status: s.status.clone(),
                        user_story: md_story.map(|m| m.user_story.clone()).unwrap_or_default(),
                        acceptance_criteria: md_story
                            .map(|m| m.acceptance_criteria.clone())
                            .unwrap_or_default(),
                    }
                })
                .collect();

            StoreEpic {
                number: epic.number,
                title: epic.title.clone(),
                status: epic.status.clone(),
                description: md.map(|m| m.description.clone()).unwrap_or_default(),
                frs_covered: md.map(|m| m.frs_covered.clone()).unwrap_or_default(),
                nfrs_covered: md.map(|m| m.nfrs_covered.clone()).unwrap_or_default(),
                stories,
            }
        })
        .collect();

    let store = BoardStore {
        version: 1,
        project,
        last_updated: if last_updated.is_empty() {
            today_string()
        } else {
            last_updated
        },
        synced_from: Some("sprint-status.yaml".to_string()),
        epics: store_epics,
        assignments: vec![],
    };

    save_store(&config.base_dir, &store)?;
    Ok(store)
}

// ── Store → BoardData conversions ────────────────────────────────────────────

pub fn get_board_data_from_store(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;

    let items: Vec<board::BoardItem> = store
        .assignments
        .iter()
        .map(|a| {
            let total_tasks = a.tasks.len();
            let done_tasks = a.tasks.iter().filter(|t| t.done).count();
            let task_progress = if total_tasks > 0 {
                Some((done_tasks as f64 / total_tasks as f64 * 100.0 * 10.0).round() / 10.0)
            } else {
                None
            };
            board::BoardItem {
                id: a.id.clone(),
                item_type: "assignment".to_string(),
                title: a.title.clone(),
                status: a.status.clone(),
                phase: 0,
                epic_id: a.assignee.clone(),
                epic_title: Some(format!("{}/{} tasks", done_tasks, total_tasks)),
                story_number: Some(a.priority.clone()),
                story_count: Some(total_tasks),
                stories_done: Some(done_tasks),
                progress_pct: task_progress,
                comment_count: Some(a.comments.len()),
                labels: if a.labels.is_empty() { None } else { Some(a.labels.clone()) },
                description: if a.description.is_empty() { None } else { Some(a.description.clone()) },
                assignee: if a.assignee.is_empty() { None } else { Some(a.assignee.clone()) },
            }
        })
        .collect();

    let total = items.len();
    let done = items.iter().filter(|i| i.status == "done").count();
    let in_progress = items.iter().filter(|i| i.status == "in-progress").count();
    let ready = items.iter().filter(|i| i.status == "ready-for-dev").count();
    let review = items.iter().filter(|i| i.status == "review").count();
    let backlog = items.iter().filter(|i| i.status == "backlog").count();
    let progress_pct = if total > 0 {
        ((done as f64 / total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let data = board::BoardData {
        project: store.project,
        last_updated: store.last_updated,
        phases: vec![],
        summary: board::BoardSummary {
            total_epics: 0,
            total_stories: total,
            done_epics: 0,
            done_stories: done,
            in_progress_stories: in_progress,
            ready_stories: ready,
            backlog_stories: backlog,
            review_stories: review,
            progress_pct,
        },
        items,
    };

    serde_json::to_value(&data)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

pub fn get_epics_list_from_store(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;

    let rows: Vec<serde_json::Value> = store
        .epics
        .iter()
        .map(|epic| {
            let phase = board::epic_phase(epic.number);
            let done = epic.stories.iter().filter(|s| s.status == "done").count();
            let total = epic.stories.len();
            let progress = if total > 0 {
                format!("{}/{} ({:.0}%)", done, total, done as f64 / total as f64 * 100.0)
            } else {
                "No stories".to_string()
            };
            serde_json::json!({
                "id": format!("epic-{}", epic.number),
                "title": epic.title,
                "status": epic.status,
                "phase_label": board::phase_label(phase),
                "progress": progress,
                "story_count": total,
                "description": epic.description,
            })
        })
        .collect();

    Ok(serde_json::json!({ "items": rows }))
}

pub fn get_assignments_list_from_store(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;

    let rows: Vec<serde_json::Value> = store
        .assignments
        .iter()
        .map(|a| {
            let total = a.tasks.len();
            let done = a.tasks.iter().filter(|t| t.done).count();
            serde_json::json!({
                "id": a.id,
                "title": a.title,
                "status": a.status,
                "assignee": if a.assignee.is_empty() { "Unassigned" } else { &a.assignee },
                "priority": a.priority,
                "task_progress": format!("{}/{}", done, total),
                "comment_count": a.comments.len(),
                "labels": a.labels,
            })
        })
        .collect();

    Ok(serde_json::json!({ "items": rows }))
}

pub fn get_assignment_detail_from_store(
    assignment_id: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;
    let assignment = store
        .assignments
        .iter()
        .find(|a| a.id == assignment_id)
        .ok_or_else(|| {
            WitPluginError::not_found(format!("Assignment '{assignment_id}' not found"))
        })?;

    let total_tasks = assignment.tasks.len();
    let done_tasks = assignment.tasks.iter().filter(|t| t.done).count();

    Ok(serde_json::json!({
        "id": assignment.id,
        "title": assignment.title,
        "status": assignment.status,
        "description": assignment.description,
        "assignee": assignment.assignee,
        "priority": assignment.priority,
        "labels": assignment.labels,
        "task_progress": format!("{}/{}", done_tasks, total_tasks),
        "tasks": assignment.tasks.iter().map(|t| serde_json::json!({
            "id": t.id,
            "title": t.title,
            "done": t.done,
        })).collect::<Vec<_>>(),
        "comments": assignment.comments.iter().map(|c| serde_json::json!({
            "id": c.id,
            "author": c.author,
            "content": c.content,
            "timestamp": c.timestamp,
        })).collect::<Vec<_>>(),
        "task_count": total_tasks,
        "tasks_done": done_tasks,
    }))
}

pub fn get_filter_options_from_store(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;

    let mut epic_options: Vec<board::FilterValue> = store
        .epics
        .iter()
        .map(|e| board::FilterValue {
            value: format!("epic-{}", e.number),
            label: format!("Epic {}: {}", e.number, e.title),
        })
        .collect();
    epic_options.sort_by(|a, b| a.value.cmp(&b.value));

    let options = board::FilterOptions {
        phases: vec![
            board::FilterValue {
                value: "1".to_string(),
                label: board::phase_label(1).to_string(),
            },
            board::FilterValue {
                value: "2".to_string(),
                label: board::phase_label(2).to_string(),
            },
            board::FilterValue {
                value: "3".to_string(),
                label: board::phase_label(3).to_string(),
            },
        ],
        epics: epic_options,
        statuses: vec![
            board::FilterValue {
                value: "backlog".to_string(),
                label: "Backlog".to_string(),
            },
            board::FilterValue {
                value: "ready-for-dev".to_string(),
                label: "Ready for Dev".to_string(),
            },
            board::FilterValue {
                value: "in-progress".to_string(),
                label: "In Progress".to_string(),
            },
            board::FilterValue {
                value: "review".to_string(),
                label: "Review".to_string(),
            },
            board::FilterValue {
                value: "done".to_string(),
                label: "Done".to_string(),
            },
        ],
        types: vec![
            board::FilterValue {
                value: "epic".to_string(),
                label: "Epic".to_string(),
            },
            board::FilterValue {
                value: "story".to_string(),
                label: "Story".to_string(),
            },
        ],
    };

    serde_json::to_value(&options)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

pub fn get_epic_detail_from_store(
    epic_id: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let epic_num: u32 = epic_id
        .strip_prefix("epic-")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| WitPluginError::not_found(format!("Invalid epic ID: '{epic_id}'")))?;

    let store = load_store(&config.base_dir)?;
    let epic = store
        .epics
        .iter()
        .find(|e| e.number == epic_num)
        .ok_or_else(|| WitPluginError::not_found(format!("Epic '{epic_id}' not found")))?;

    let phase = board::epic_phase(epic_num);
    let done_count = epic.stories.iter().filter(|s| s.status == "done").count();
    let in_progress_count = epic
        .stories
        .iter()
        .filter(|s| s.status == "in-progress")
        .count();

    let progress = if epic.stories.is_empty() {
        "No stories".to_string()
    } else {
        format!(
            "{}/{} done ({:.0}%)",
            done_count,
            epic.stories.len(),
            done_count as f64 / epic.stories.len() as f64 * 100.0
        )
    };

    let story_list: Vec<board::StoryListItem> = epic
        .stories
        .iter()
        .map(|s| board::StoryListItem {
            id: s.id.clone(),
            story_number: s.story_number.clone(),
            title: s.title.clone(),
            status: s.status.clone(),
        })
        .collect();

    let detail = board::EpicDetail {
        id: epic_id.to_string(),
        number: epic_num,
        title: epic.title.clone(),
        description: epic.description.clone(),
        status: epic.status.clone(),
        phase,
        phase_label: board::phase_label(phase).to_string(),
        frs_covered: epic.frs_covered.clone(),
        nfrs_covered: epic.nfrs_covered.clone(),
        story_count: epic.stories.len(),
        stories_done: done_count,
        stories_in_progress: in_progress_count,
        progress,
        story_list,
    };

    serde_json::to_value(&detail)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

pub fn get_story_detail_from_store(
    story_id: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;

    for epic in &store.epics {
        if let Some(story) = epic.stories.iter().find(|s| s.id == story_id) {
            let phase = board::epic_phase(epic.number);
            let detail = board::StoryDetail {
                id: story.id.clone(),
                story_number: story.story_number.clone(),
                title: story.title.clone(),
                status: story.status.clone(),
                epic_id: format!("epic-{}", epic.number),
                epic_title: epic.title.clone(),
                phase,
                phase_label: board::phase_label(phase).to_string(),
                user_story: story.user_story.clone(),
                acceptance_criteria: story.acceptance_criteria.clone(),
            };
            return serde_json::to_value(&detail)
                .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")));
        }
    }

    Err(WitPluginError::not_found(format!(
        "Story '{story_id}' not found"
    )))
}

pub fn get_board_summary_from_store(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let store = load_store(&config.base_dir)?;

    let total_stories: usize = store.epics.iter().map(|e| e.stories.len()).sum();
    let done_stories: usize = store
        .epics
        .iter()
        .flat_map(|e| &e.stories)
        .filter(|s| s.status == "done")
        .count();
    let remaining = total_stories - done_stories;
    let progress_pct = if total_stories > 0 {
        ((done_stories as f64 / total_stories as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let current_phase = store
        .epics
        .iter()
        .filter(|e| e.status != "done")
        .map(|e| board::epic_phase(e.number))
        .max()
        .unwrap_or(3);

    let has_active_work = remaining == 0
        || store.epics.iter().any(|e| {
            e.stories
                .iter()
                .any(|s| s.status == "in-progress" || s.status == "review")
        });
    let sprint_progress = if has_active_work {
        "on-track"
    } else {
        "at-risk"
    };

    let summary = board::BoardSummaryCompact {
        sprint_progress: sprint_progress.to_string(),
        progress_pct,
        stories_remaining: remaining,
        current_phase: board::phase_label(current_phase).to_string(),
    };

    serde_json::to_value(&summary)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

// ── Mutation operations ──────────────────────────────────────────────────────

pub fn update_item_status(
    base_dir: &Path,
    item_id: &str,
    new_status: &str,
) -> Result<serde_json::Value, WitPluginError> {
    validate_status(new_status)?;
    let mut store = load_store(base_dir)?;

    if let Some(epic_num_str) = item_id.strip_prefix("epic-") {
        let epic_num: u32 = epic_num_str
            .parse()
            .map_err(|_| WitPluginError::not_found(format!("Invalid epic ID: '{item_id}'")))?;
        let epic = store
            .epics
            .iter_mut()
            .find(|e| e.number == epic_num)
            .ok_or_else(|| WitPluginError::not_found(format!("Epic '{item_id}' not found")))?;
        epic.status = new_status.to_string();
        store.last_updated = today_string();
        save_store(base_dir, &store)?;
        return Ok(serde_json::json!({
            "id": item_id,
            "type": "epic",
            "status": new_status
        }));
    }

    // Story
    for epic in &mut store.epics {
        if let Some(story) = epic.stories.iter_mut().find(|s| s.id == item_id) {
            story.status = new_status.to_string();
            store.last_updated = today_string();
            save_store(base_dir, &store)?;
            return Ok(serde_json::json!({
                "id": item_id,
                "type": "story",
                "status": new_status
            }));
        }
    }

    Err(WitPluginError::not_found(format!(
        "Item '{item_id}' not found"
    )))
}

pub fn create_epic(
    base_dir: &Path,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'title' field required"))?;

    let mut store = load_store(base_dir)?;
    let next_number = store.epics.iter().map(|e| e.number).max().unwrap_or(0) + 1;

    let epic = StoreEpic {
        number: next_number,
        title: title.to_string(),
        status: payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("backlog")
            .to_string(),
        description: payload
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        frs_covered: payload
            .get("frs_covered")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        nfrs_covered: payload
            .get("nfrs_covered")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        stories: vec![],
    };

    if epic.status != "backlog" {
        validate_status(&epic.status)?;
    }

    let result = serde_json::to_value(&epic)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    store.epics.push(epic);
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

pub fn update_epic(
    base_dir: &Path,
    epic_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let epic_num: u32 = epic_id
        .strip_prefix("epic-")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| WitPluginError::not_found(format!("Invalid epic ID: '{epic_id}'")))?;

    let mut store = load_store(base_dir)?;
    let epic = store
        .epics
        .iter_mut()
        .find(|e| e.number == epic_num)
        .ok_or_else(|| WitPluginError::not_found(format!("Epic '{epic_id}' not found")))?;

    if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
        epic.title = title.to_string();
    }
    if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
        validate_status(status)?;
        epic.status = status.to_string();
    }
    if let Some(desc) = payload.get("description").and_then(|v| v.as_str()) {
        epic.description = desc.to_string();
    }
    if let Some(frs) = payload.get("frs_covered").and_then(|v| v.as_str()) {
        epic.frs_covered = frs.to_string();
    }
    if let Some(nfrs) = payload.get("nfrs_covered").and_then(|v| v.as_str()) {
        epic.nfrs_covered = nfrs.to_string();
    }

    let result = serde_json::to_value(&*epic)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

pub fn create_story(
    base_dir: &Path,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let epic_id = payload
        .get("epic_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'epic_id' field required"))?;
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'title' field required"))?;

    let epic_num: u32 = epic_id
        .strip_prefix("epic-")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| WitPluginError::invalid_input(format!("Invalid epic_id: '{epic_id}'")))?;

    let mut store = load_store(base_dir)?;
    let epic = store
        .epics
        .iter_mut()
        .find(|e| e.number == epic_num)
        .ok_or_else(|| WitPluginError::not_found(format!("Epic '{epic_id}' not found")))?;

    // Auto-assign next story number
    let next_story_num = epic
        .stories
        .iter()
        .filter_map(|s| {
            s.story_number
                .split('.')
                .nth(1)
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        + 1;

    let story_number = format!("{}.{}", epic_num, next_story_num);
    let slug = title_to_slug(title);
    let id = format!("{}-{}-{}", epic_num, next_story_num, slug);

    let story = StoreStory {
        id: id.clone(),
        story_number,
        title: title.to_string(),
        status: payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("backlog")
            .to_string(),
        user_story: payload
            .get("user_story")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        acceptance_criteria: payload
            .get("acceptance_criteria")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    };

    if story.status != "backlog" {
        validate_status(&story.status)?;
    }

    let result = serde_json::to_value(&story)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    epic.stories.push(story);
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

pub fn update_story(
    base_dir: &Path,
    story_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let mut store = load_store(base_dir)?;

    for epic in &mut store.epics {
        if let Some(story) = epic.stories.iter_mut().find(|s| s.id == story_id) {
            if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
                story.title = title.to_string();
            }
            if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
                validate_status(status)?;
                story.status = status.to_string();
            }
            if let Some(us) = payload.get("user_story").and_then(|v| v.as_str()) {
                story.user_story = us.to_string();
            }
            if let Some(ac) = payload.get("acceptance_criteria").and_then(|v| v.as_str()) {
                story.acceptance_criteria = ac.to_string();
            }

            let result = serde_json::to_value(&*story)
                .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
            store.last_updated = today_string();
            save_store(base_dir, &store)?;
            return Ok(result);
        }
    }

    Err(WitPluginError::not_found(format!(
        "Story '{story_id}' not found"
    )))
}

// ── Assignment CRUD ─────────────────────────────────────────────────────────

/// Ensure store exists (create empty if not).
fn ensure_store(base_dir: &Path) -> Result<BoardStore, WitPluginError> {
    if store_exists(base_dir) {
        load_store(base_dir)
    } else {
        let store = BoardStore {
            version: 1,
            project: "pulse".to_string(),
            last_updated: today_string(),
            synced_from: None,
            epics: vec![],
            assignments: vec![],
        };
        save_store(base_dir, &store)?;
        Ok(store)
    }
}

/// Create a new assignment (task card on the board).
pub fn create_assignment(
    base_dir: &Path,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'title' field required"))?;

    let status = payload
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("backlog");
    validate_status(status)?;

    let mut store = ensure_store(base_dir)?;

    // Auto-generate ID from title slug + counter
    let slug = title_to_slug(title);
    let next_num = store.assignments.len() + 1;
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("task-{}-{}", next_num, slug));

    let assignment = StoreAssignment {
        id: id.clone(),
        title: title.to_string(),
        status: status.to_string(),
        description: payload
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        assignee: payload
            .get("assignee")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        priority: payload
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
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
        tasks: payload
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
            .unwrap_or_default(),
        comments: vec![],
        workflow_id: payload
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    };

    let result = serde_json::to_value(&assignment)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    store.assignments.push(assignment);
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

/// Update an existing assignment's fields.
pub fn update_assignment(
    base_dir: &Path,
    assignment_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let mut store = load_store(base_dir)?;
    let assignment = store
        .assignments
        .iter_mut()
        .find(|a| a.id == assignment_id)
        .ok_or_else(|| {
            WitPluginError::not_found(format!("Assignment '{assignment_id}' not found"))
        })?;

    if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
        assignment.title = title.to_string();
    }
    if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
        validate_status(status)?;
        assignment.status = status.to_string();
    }
    if let Some(desc) = payload.get("description").and_then(|v| v.as_str()) {
        assignment.description = desc.to_string();
    }
    if let Some(assignee) = payload.get("assignee").and_then(|v| v.as_str()) {
        assignment.assignee = assignee.to_string();
    }
    if let Some(priority) = payload.get("priority").and_then(|v| v.as_str()) {
        assignment.priority = priority.to_string();
    }
    if let Some(labels) = payload.get("labels").and_then(|v| v.as_array()) {
        assignment.labels = labels
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }

    let result = serde_json::to_value(&*assignment)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

/// Add a comment to an assignment.
pub fn add_comment(
    base_dir: &Path,
    assignment_id: &str,
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

    let mut store = load_store(base_dir)?;
    let assignment = store
        .assignments
        .iter_mut()
        .find(|a| a.id == assignment_id)
        .ok_or_else(|| {
            WitPluginError::not_found(format!("Assignment '{assignment_id}' not found"))
        })?;

    let comment_num = assignment.comments.len() + 1;
    let comment = Comment {
        id: format!("comment-{}", comment_num),
        author: author.to_string(),
        content: content.to_string(),
        timestamp: today_string(),
    };

    let result = serde_json::to_value(&comment)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    assignment.comments.push(comment);
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

/// Add a sub-task to an assignment.
pub fn add_subtask(
    base_dir: &Path,
    assignment_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WitPluginError> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WitPluginError::invalid_input("'title' field required"))?;

    let mut store = load_store(base_dir)?;
    let assignment = store
        .assignments
        .iter_mut()
        .find(|a| a.id == assignment_id)
        .ok_or_else(|| {
            WitPluginError::not_found(format!("Assignment '{assignment_id}' not found"))
        })?;

    let st_num = assignment.tasks.len() + 1;
    let subtask = SubTask {
        id: format!("st-{}", st_num),
        title: title.to_string(),
        done: false,
    };

    let result = serde_json::to_value(&subtask)
        .map_err(|e| WitPluginError::internal(format!("JSON error: {e}")))?;
    assignment.tasks.push(subtask);
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

/// Toggle a sub-task's done status.
pub fn toggle_subtask(
    base_dir: &Path,
    assignment_id: &str,
    subtask_id: &str,
) -> Result<serde_json::Value, WitPluginError> {
    let mut store = load_store(base_dir)?;
    let assignment = store
        .assignments
        .iter_mut()
        .find(|a| a.id == assignment_id)
        .ok_or_else(|| {
            WitPluginError::not_found(format!("Assignment '{assignment_id}' not found"))
        })?;

    let subtask = assignment
        .tasks
        .iter_mut()
        .find(|t| t.id == subtask_id)
        .ok_or_else(|| {
            WitPluginError::not_found(format!("Subtask '{subtask_id}' not found"))
        })?;

    subtask.done = !subtask.done;

    let result = serde_json::json!({
        "id": subtask.id,
        "title": subtask.title,
        "done": subtask.done,
    });
    store.last_updated = today_string();
    save_store(base_dir, &store)?;
    Ok(result)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> BoardStore {
        BoardStore {
            version: 1,
            project: "test-project".to_string(),
            last_updated: "2026-03-27".to_string(),
            synced_from: None,
            assignments: vec![],
            epics: vec![StoreEpic {
                number: 1,
                title: "Test Epic".to_string(),
                status: "in-progress".to_string(),
                description: "Test description".to_string(),
                frs_covered: "FR-1".to_string(),
                nfrs_covered: "NFR-1".to_string(),
                stories: vec![
                    StoreStory {
                        id: "1-1-first-story".to_string(),
                        story_number: "1.1".to_string(),
                        title: "First Story".to_string(),
                        status: "done".to_string(),
                        user_story: "As a user...".to_string(),
                        acceptance_criteria: "- criterion 1".to_string(),
                    },
                    StoreStory {
                        id: "1-2-second-story".to_string(),
                        story_number: "1.2".to_string(),
                        title: "Second Story".to_string(),
                        status: "backlog".to_string(),
                        user_story: String::new(),
                        acceptance_criteria: String::new(),
                    },
                ],
            }],
        }
    }

    #[test]
    fn test_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let store = test_store();

        save_store(base, &store).unwrap();
        assert!(store_exists(base));

        let loaded = load_store(base).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.project, "test-project");
        assert_eq!(loaded.epics.len(), 1);
        assert_eq!(loaded.epics[0].stories.len(), 2);
        assert_eq!(loaded.epics[0].stories[0].id, "1-1-first-story");
    }

    #[test]
    fn test_store_not_found() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!store_exists(dir.path()));
        assert!(load_store(dir.path()).is_err());
    }

    #[test]
    fn test_atomic_write_no_tmp_left() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let tmp = store_path(base).with_extension("json.tmp");
        assert!(!tmp.exists(), ".tmp file should not remain");
    }

    #[test]
    fn test_update_item_status_story() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = update_item_status(base, "1-2-second-story", "in-progress").unwrap();
        assert_eq!(result["status"], "in-progress");

        let loaded = load_store(base).unwrap();
        let story = &loaded.epics[0].stories[1];
        assert_eq!(story.status, "in-progress");
    }

    #[test]
    fn test_update_item_status_epic() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = update_item_status(base, "epic-1", "done").unwrap();
        assert_eq!(result["status"], "done");
        assert_eq!(result["type"], "epic");
    }

    #[test]
    fn test_update_status_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err = update_item_status(base, "epic-1", "invalid-status").unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn test_update_status_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err = update_item_status(base, "nonexistent", "done").unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_create_epic() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = create_epic(
            base,
            &serde_json::json!({
                "title": "New Epic",
                "description": "A new epic"
            }),
        )
        .unwrap();

        assert_eq!(result["number"], 2);
        assert_eq!(result["title"], "New Epic");
        assert_eq!(result["status"], "backlog");

        let loaded = load_store(base).unwrap();
        assert_eq!(loaded.epics.len(), 2);
    }

    #[test]
    fn test_update_epic_partial() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = update_epic(
            base,
            "epic-1",
            &serde_json::json!({"title": "Updated Title"}),
        )
        .unwrap();

        assert_eq!(result["title"], "Updated Title");
        assert_eq!(result["description"], "Test description"); // unchanged
    }

    #[test]
    fn test_create_story() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = create_story(
            base,
            &serde_json::json!({
                "epic_id": "epic-1",
                "title": "Third Story"
            }),
        )
        .unwrap();

        assert_eq!(result["story_number"], "1.3");
        assert_eq!(result["status"], "backlog");
        assert!(result["id"].as_str().unwrap().starts_with("1-3-"));

        let loaded = load_store(base).unwrap();
        assert_eq!(loaded.epics[0].stories.len(), 3);
    }

    #[test]
    fn test_create_story_epic_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err = create_story(
            base,
            &serde_json::json!({"epic_id": "epic-999", "title": "X"}),
        )
        .unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_update_story_partial() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = update_story(
            base,
            "1-1-first-story",
            &serde_json::json!({"status": "review"}),
        )
        .unwrap();

        assert_eq!(result["status"], "review");
        assert_eq!(result["title"], "First Story"); // unchanged
    }

    #[test]
    fn test_sync_from_artifacts() {
        let config = WorkspaceConfig::resolve(None);
        let result = sync_from_artifacts(&config);
        assert!(result.is_ok(), "sync failed: {:?}", result.err());

        let store = result.unwrap();
        assert_eq!(store.project, "bmad-method-flow");
        assert!(store.epics.len() >= 21);
        assert_eq!(store.synced_from, Some("sprint-status.yaml".to_string()));

        // Verify store file was created
        assert!(store_exists(&config.base_dir));

        // Clean up
        let _ = std::fs::remove_file(store_path(&config.base_dir));
    }

    #[test]
    fn test_title_to_slug() {
        assert_eq!(title_to_slug("My New Feature"), "my-new-feature");
        assert_eq!(
            title_to_slug("Crate Scaffolding & Setup"),
            "crate-scaffolding-setup"
        );
        assert_eq!(title_to_slug(""), "");
    }

    #[test]
    fn test_today_string_format() {
        let today = today_string();
        assert!(today.len() == 10, "expected YYYY-MM-DD, got: {today}");
        assert!(today.contains('-'));
    }

    // ── Helper with assignments ──────────────────────────────────────────

    fn test_store_with_assignments() -> BoardStore {
        let mut store = test_store();
        store.assignments = vec![
            StoreAssignment {
                id: "task-1-setup".to_string(),
                title: "Setup CI".to_string(),
                status: "in-progress".to_string(),
                description: "Configure CI pipeline".to_string(),
                assignee: "dev-1".to_string(),
                priority: "high".to_string(),
                labels: vec!["infra".to_string()],
                tasks: vec![
                    SubTask {
                        id: "st-1".to_string(),
                        title: "Add Dockerfile".to_string(),
                        done: true,
                    },
                    SubTask {
                        id: "st-2".to_string(),
                        title: "Add CI config".to_string(),
                        done: false,
                    },
                ],
                comments: vec![Comment {
                    id: "comment-1".to_string(),
                    author: "LLM Agent".to_string(),
                    content: "Started work".to_string(),
                    timestamp: "2026-03-27".to_string(),
                }],
                workflow_id: String::new(),
            },
            StoreAssignment {
                id: "task-2-tests".to_string(),
                title: "Write Tests".to_string(),
                status: "done".to_string(),
                description: "Add unit tests".to_string(),
                assignee: "dev-2".to_string(),
                priority: "medium".to_string(),
                labels: vec!["testing".to_string()],
                tasks: vec![],
                comments: vec![],
                workflow_id: String::new(),
            },
        ];
        store
    }

    // ── Assignment CRUD tests ────────────────────────────────────────────

    #[test]
    fn test_create_assignment() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = create_assignment(
            base,
            &serde_json::json!({
                "title": "New Task",
                "description": "Do something",
                "assignee": "dev-1",
                "priority": "high",
                "labels": ["bug", "urgent"],
                "tasks": ["subtask A", "subtask B"]
            }),
        )
        .unwrap();

        assert_eq!(result["title"], "New Task");
        assert_eq!(result["status"], "backlog");
        assert_eq!(result["priority"], "high");
        assert_eq!(result["labels"].as_array().unwrap().len(), 2);
        assert_eq!(result["tasks"].as_array().unwrap().len(), 2);
        assert_eq!(result["tasks"][0]["id"], "st-1");
        assert!(!result["tasks"][0]["done"].as_bool().unwrap());

        let loaded = load_store(base).unwrap();
        assert_eq!(loaded.assignments.len(), 1);
    }

    #[test]
    fn test_create_assignment_minimal() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = create_assignment(base, &serde_json::json!({"title": "Minimal"})).unwrap();
        assert_eq!(result["title"], "Minimal");
        assert_eq!(result["status"], "backlog");
        assert_eq!(result["priority"], "medium");
        assert_eq!(result["assignee"], "");
    }

    #[test]
    fn test_create_assignment_invalid_status() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err = create_assignment(
            base,
            &serde_json::json!({"title": "X", "status": "bad"}),
        )
        .unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn test_create_assignment_missing_title() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err = create_assignment(base, &serde_json::json!({"status": "backlog"})).unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn test_create_assignment_auto_creates_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        // No store exists yet — ensure_store should create one
        let result =
            create_assignment(base, &serde_json::json!({"title": "First Task"})).unwrap();
        assert_eq!(result["title"], "First Task");
        assert!(store_exists(base));
    }

    #[test]
    fn test_update_assignment() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let result = update_assignment(
            base,
            "task-1-setup",
            &serde_json::json!({
                "title": "Updated Title",
                "status": "review",
                "priority": "critical"
            }),
        )
        .unwrap();

        assert_eq!(result["title"], "Updated Title");
        assert_eq!(result["status"], "review");
        assert_eq!(result["priority"], "critical");
        assert_eq!(result["description"], "Configure CI pipeline"); // unchanged
    }

    #[test]
    fn test_update_assignment_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err =
            update_assignment(base, "nonexistent", &serde_json::json!({"title": "X"})).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_update_assignment_invalid_status() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err = update_assignment(
            base,
            "task-1-setup",
            &serde_json::json!({"status": "invalid"}),
        )
        .unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    // ── Comment tests ────────────────────────────────────────────────────

    #[test]
    fn test_add_comment() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let result = add_comment(
            base,
            "task-1-setup",
            &serde_json::json!({"content": "Work in progress", "author": "Jack"}),
        )
        .unwrap();

        assert_eq!(result["author"], "Jack");
        assert_eq!(result["content"], "Work in progress");
        assert_eq!(result["id"], "comment-2"); // existing has comment-1

        let loaded = load_store(base).unwrap();
        assert_eq!(loaded.assignments[0].comments.len(), 2);
    }

    #[test]
    fn test_add_comment_default_author() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let result = add_comment(
            base,
            "task-1-setup",
            &serde_json::json!({"content": "Auto comment"}),
        )
        .unwrap();

        assert_eq!(result["author"], "LLM Agent");
    }

    #[test]
    fn test_add_comment_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err = add_comment(
            base,
            "nonexistent",
            &serde_json::json!({"content": "X"}),
        )
        .unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_add_comment_missing_content() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err = add_comment(
            base,
            "task-1-setup",
            &serde_json::json!({"author": "X"}),
        )
        .unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    // ── Subtask tests ────────────────────────────────────────────────────

    #[test]
    fn test_add_subtask() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let result = add_subtask(
            base,
            "task-1-setup",
            &serde_json::json!({"title": "New subtask"}),
        )
        .unwrap();

        assert_eq!(result["title"], "New subtask");
        assert_eq!(result["id"], "st-3"); // existing has st-1, st-2
        assert!(!result["done"].as_bool().unwrap());

        let loaded = load_store(base).unwrap();
        assert_eq!(loaded.assignments[0].tasks.len(), 3);
    }

    #[test]
    fn test_add_subtask_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err = add_subtask(
            base,
            "nonexistent",
            &serde_json::json!({"title": "X"}),
        )
        .unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_toggle_subtask() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        // st-1 starts as done=true, toggle to false
        let result = toggle_subtask(base, "task-1-setup", "st-1").unwrap();
        assert!(!result["done"].as_bool().unwrap());

        // Toggle again back to true
        let result = toggle_subtask(base, "task-1-setup", "st-1").unwrap();
        assert!(result["done"].as_bool().unwrap());
    }

    #[test]
    fn test_toggle_subtask_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err = toggle_subtask(base, "task-1-setup", "nonexistent").unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_toggle_subtask_assignment_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();

        let err = toggle_subtask(base, "nonexistent", "st-1").unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    // ── Store → BoardData conversion tests ───────────────────────────────

    #[test]
    fn test_get_board_data_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_board_data_from_store(&config).unwrap();
        let items = data["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(data["summary"]["total_stories"], 2);
        assert_eq!(data["summary"]["done_stories"], 1);
        assert_eq!(items[0]["type"], "assignment");
    }

    #[test]
    fn test_get_epics_list_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_epics_list_from_store(&config).unwrap();
        let items = data["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "Test Epic");
        assert!(items[0]["progress"].as_str().unwrap().contains("1/2"));
    }

    #[test]
    fn test_get_assignments_list_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_assignments_list_from_store(&config).unwrap();
        let items = data["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["title"], "Setup CI");
        assert_eq!(items[0]["task_progress"], "1/2");
        assert_eq!(items[0]["comment_count"], 1);
        assert_eq!(items[1]["assignee"], "dev-2");
    }

    #[test]
    fn test_get_assignment_detail_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_assignment_detail_from_store("task-1-setup", &config).unwrap();
        assert_eq!(data["title"], "Setup CI");
        assert_eq!(data["task_progress"], "1/2");
        assert_eq!(data["task_count"], 2);
        assert_eq!(data["tasks_done"], 1);
        assert_eq!(data["tasks"].as_array().unwrap().len(), 2);
        assert_eq!(data["comments"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_get_assignment_detail_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store_with_assignments()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let err = get_assignment_detail_from_store("nonexistent", &config).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_get_epic_detail_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_epic_detail_from_store("epic-1", &config).unwrap();
        assert_eq!(data["title"], "Test Epic");
        assert_eq!(data["story_count"], 2);
        assert_eq!(data["stories_done"], 1);
        assert!(data["progress"].as_str().unwrap().contains("1/2"));
        assert_eq!(data["story_list"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_get_epic_detail_invalid_id() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let err = get_epic_detail_from_store("bad-id", &config).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_get_story_detail_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_story_detail_from_store("1-1-first-story", &config).unwrap();
        assert_eq!(data["title"], "First Story");
        assert_eq!(data["epic_id"], "epic-1");
        assert_eq!(data["epic_title"], "Test Epic");
        assert_eq!(data["user_story"], "As a user...");
    }

    #[test]
    fn test_get_story_detail_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let err = get_story_detail_from_store("nonexistent", &config).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_get_filter_options_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_filter_options_from_store(&config).unwrap();
        assert_eq!(data["phases"].as_array().unwrap().len(), 3);
        assert_eq!(data["statuses"].as_array().unwrap().len(), 5);
        assert_eq!(data["types"].as_array().unwrap().len(), 2);
        assert_eq!(data["epics"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_get_board_summary_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_board_summary_from_store(&config).unwrap();
        assert_eq!(data["stories_remaining"], 1);
        assert_eq!(data["progress_pct"], 50.0);
        assert_eq!(data["sprint_progress"], "at-risk"); // no in-progress/review stories after 1 remaining
    }

    #[test]
    fn test_get_board_summary_all_done() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let mut store = test_store();
        store.epics[0].stories[1].status = "done".to_string();
        store.epics[0].status = "done".to_string();
        save_store(base, &store).unwrap();
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_board_summary_from_store(&config).unwrap();
        assert_eq!(data["stories_remaining"], 0);
        assert_eq!(data["progress_pct"], 100.0);
        assert_eq!(data["sprint_progress"], "on-track");
    }

    // ── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn test_validate_status_all_valid() {
        for status in VALID_STATUSES {
            assert!(validate_status(status).is_ok());
        }
    }

    #[test]
    fn test_update_epic_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err = update_epic(base, "epic-999", &serde_json::json!({"title": "X"})).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_update_story_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err =
            update_story(base, "nonexistent", &serde_json::json!({"title": "X"})).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_create_epic_with_status() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = create_epic(
            base,
            &serde_json::json!({"title": "WIP Epic", "status": "in-progress"}),
        )
        .unwrap();
        assert_eq!(result["status"], "in-progress");
    }

    #[test]
    fn test_create_epic_invalid_status() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let err =
            create_epic(base, &serde_json::json!({"title": "X", "status": "bad"})).unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn test_create_story_with_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap();

        let result = create_story(
            base,
            &serde_json::json!({
                "epic_id": "epic-1",
                "title": "Full Story",
                "status": "ready-for-dev",
                "user_story": "As a dev...",
                "acceptance_criteria": "- AC1\n- AC2"
            }),
        )
        .unwrap();

        assert_eq!(result["status"], "ready-for-dev");
        assert_eq!(result["user_story"], "As a dev...");
        assert_eq!(result["acceptance_criteria"], "- AC1\n- AC2");
    }

    #[test]
    fn test_board_data_empty_assignments() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        save_store(base, &test_store()).unwrap(); // no assignments
        let config = WorkspaceConfig {
            base_dir: base.to_path_buf(),
            ..Default::default()
        };

        let data = get_board_data_from_store(&config).unwrap();
        assert_eq!(data["items"].as_array().unwrap().len(), 0);
        assert_eq!(data["summary"]["progress_pct"], 0.0);
    }
}
