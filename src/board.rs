//! Scrum Board data module.
//!
//! Parses `sprint-status.yaml` and epic markdown files to produce structured
//! board data for the dashboard Kanban view.

use crate::workspace::WorkspaceConfig;
use pulse_plugin_sdk::error::WitPluginError;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

// ── Constants ────────────────────────────────────────────────────────────────

const SPRINT_STATUS_PATH: &str = "_bmad-output/implementation-artifacts/sprint-status.yaml";
const EPICS_PATHS: &[&str] = &[
    "_bmad-output/planning-artifacts/epics.md",
    "_bmad-output/planning-artifacts/epics-config-injection.md",
];

/// Derive phase from epic number.
fn epic_phase(epic_num: u32) -> u32 {
    match epic_num {
        1..=11 => 1,
        12..=17 => 2,
        18..=21 => 3,
        _ => 0,
    }
}

fn phase_label(phase: u32) -> &'static str {
    match phase {
        1 => "Phase 1: Core Plugin Development",
        2 => "Phase 2: SDK Integration & Agent Mesh",
        3 => "Phase 3: Config Injection & Tool Provider",
        _ => "Unknown Phase",
    }
}

// ── Serializable structs ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BoardData {
    pub project: String,
    pub last_updated: String,
    pub phases: Vec<Phase>,
    pub summary: BoardSummary,
    pub items: Vec<BoardItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub id: u32,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardSummary {
    pub total_epics: usize,
    pub total_stories: usize,
    pub done_epics: usize,
    pub done_stories: usize,
    pub in_progress_stories: usize,
    pub ready_stories: usize,
    pub backlog_stories: usize,
    pub review_stories: usize,
    pub progress_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub title: String,
    pub status: String,
    pub phase: u32,
    pub epic_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_number: Option<String>,
    // Epic-only fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stories_done: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterOptions {
    pub phases: Vec<FilterValue>,
    pub epics: Vec<FilterValue>,
    pub statuses: Vec<FilterValue>,
    pub types: Vec<FilterValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterValue {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpicDetail {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub description: String,
    pub status: String,
    pub phase: u32,
    pub phase_label: String,
    pub frs_covered: String,
    pub nfrs_covered: String,
    pub story_count: usize,
    pub stories_done: usize,
    pub stories_in_progress: usize,
    pub progress: String,
    pub story_list: Vec<StoryListItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoryListItem {
    pub id: String,
    pub story_number: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoryDetail {
    pub id: String,
    pub story_number: String,
    pub title: String,
    pub status: String,
    pub epic_id: String,
    pub epic_title: String,
    pub phase: u32,
    pub phase_label: String,
    pub user_story: String,
    pub acceptance_criteria: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardSummaryCompact {
    pub sprint_progress: String,
    pub progress_pct: f64,
    pub stories_remaining: usize,
    pub current_phase: String,
}

// ── Internal parsing types ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ParsedEpic {
    number: u32,
    title: String,
    status: String,
    stories: Vec<ParsedStory>,
}

#[derive(Debug, Clone)]
struct ParsedStory {
    id: String,
    epic_number: u32,
    story_number: String,
    title: String,
    status: String,
}

#[derive(Debug, Clone, Default)]
struct EpicMetadata {
    description: String,
    frs_covered: String,
    nfrs_covered: String,
    stories: BTreeMap<String, StoryMetadata>,
}

#[derive(Debug, Clone, Default)]
struct StoryMetadata {
    user_story: String,
    acceptance_criteria: String,
}

// ── YAML parsing ─────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct SprintStatusYaml {
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default)]
    development_status: BTreeMap<String, serde_yaml::Value>,
}

/// Extract `# Epic N: Title` comment lines from raw YAML text.
fn extract_epic_titles_from_comments(raw: &str) -> BTreeMap<u32, String> {
    let mut titles = BTreeMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# Epic ") {
            // Format: "# Epic 1: Claude Code Executor Plugin (claude-code-v2)"
            if let Some(colon_pos) = rest.find(':') {
                if let Ok(num) = rest[..colon_pos].trim().parse::<u32>() {
                    let title = rest[colon_pos + 1..].trim().to_string();
                    titles.insert(num, title);
                }
            }
        }
    }
    titles
}

/// Convert a story slug like "1-1-crate-scaffolding-and-process-manager" into
/// (epic_num, story_num, title).
fn parse_story_key(key: &str) -> Option<(u32, u32, String)> {
    let mut parts = key.splitn(3, '-');
    let epic_num: u32 = parts.next()?.parse().ok()?;
    let story_num: u32 = parts.next()?.parse().ok()?;
    let slug = parts.next().unwrap_or("");
    let title = slug_to_title(slug);
    Some((epic_num, story_num, title))
}

/// Convert "crate-scaffolding-and-process-manager" → "Crate Scaffolding and Process Manager"
fn slug_to_title(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            // Keep short words lowercase for readability
            match word {
                "and" | "or" | "the" | "a" | "an" | "in" | "on" | "of" | "for" | "to" | "via"
                | "with" | "from" => word.to_string(),
                _ => {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse sprint-status.yaml into structured epic/story data.
fn parse_sprint_status(
    base_dir: &Path,
) -> Result<(Vec<ParsedEpic>, String, String), WitPluginError> {
    let path = base_dir.join(SPRINT_STATUS_PATH);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| WitPluginError::internal(format!("Cannot read sprint-status.yaml: {e}")))?;

    let yaml: SprintStatusYaml = serde_yaml::from_str(&raw)
        .map_err(|e| WitPluginError::internal(format!("Invalid sprint-status.yaml: {e}")))?;

    let project = yaml.project.unwrap_or_else(|| "unknown".to_string());
    let last_updated = yaml.last_updated.map(|v| v.to_string()).unwrap_or_default();

    let comment_titles = extract_epic_titles_from_comments(&raw);

    // Collect epic statuses and story statuses
    let mut epic_statuses: BTreeMap<u32, String> = BTreeMap::new();
    let mut stories: Vec<ParsedStory> = Vec::new();

    for (key, value) in &yaml.development_status {
        let status_str = match value {
            serde_yaml::Value::String(s) => s.clone(),
            _ => value.as_str().unwrap_or("unknown").to_string(),
        };

        if let Some(num_str) = key.strip_prefix("epic-") {
            // Epic entry: "epic-1" → epic number 1
            if !num_str.contains("-retrospective") {
                if let Ok(num) = num_str.parse::<u32>() {
                    epic_statuses.insert(num, status_str);
                }
            }
            // Skip retrospective entries
        } else if key.contains("-retrospective") {
            // Skip "epic-N-retrospective" entries
        } else if let Some((epic_num, story_num, title)) = parse_story_key(key) {
            stories.push(ParsedStory {
                id: key.clone(),
                epic_number: epic_num,
                story_number: format!("{}.{}", epic_num, story_num),
                title,
                status: status_str,
            });
        }
    }

    // Build epics with their stories
    let mut epics: Vec<ParsedEpic> = Vec::new();
    for (num, status) in &epic_statuses {
        let title = comment_titles
            .get(num)
            .cloned()
            .unwrap_or_else(|| format!("Epic {}", num));
        let epic_stories: Vec<ParsedStory> = stories
            .iter()
            .filter(|s| s.epic_number == *num)
            .cloned()
            .collect();
        epics.push(ParsedEpic {
            number: *num,
            title,
            status: status.clone(),
            stories: epic_stories,
        });
    }

    Ok((epics, project, last_updated))
}

// ── Markdown parsing ─────────────────────────────────────────────────────────

/// Parse epic markdown files to extract metadata (descriptions, FRs, user stories).
fn parse_epics_markdown(base_dir: &Path) -> BTreeMap<u32, EpicMetadata> {
    let mut metadata: BTreeMap<u32, EpicMetadata> = BTreeMap::new();

    for rel_path in EPICS_PATHS {
        let path = base_dir.join(rel_path);
        if let Ok(content) = std::fs::read_to_string(&path) {
            parse_single_epics_file(&content, &mut metadata);
        }
    }

    metadata
}

fn parse_single_epics_file(content: &str, metadata: &mut BTreeMap<u32, EpicMetadata>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Look for "## Epic N: Title" (full epic sections, not the summary list)
        // The summary list uses "### Epic N:" but full sections use "## Epic N:"
        if line.starts_with("## Epic ") && !line.starts_with("## Epic List") {
            if let Some(epic_num) = parse_epic_heading_number(line) {
                let mut meta = EpicMetadata::default();
                i += 1;

                // Collect description (lines until next heading or **FRs**)
                let mut desc_lines = Vec::new();
                while i < lines.len() {
                    let l = lines[i];
                    if l.starts_with("## ")
                        || l.starts_with("### Story")
                        || l.starts_with("**FRs")
                        || l.starts_with("**NFRs")
                    {
                        break;
                    }
                    if !l.trim().is_empty() {
                        desc_lines.push(l.trim());
                    }
                    i += 1;
                }
                meta.description = desc_lines.join(" ");

                // Look for stories within this epic section
                while i < lines.len() {
                    let l = lines[i];
                    if l.starts_with("## ") && !l.starts_with("## Epic List") {
                        // Next epic section
                        break;
                    }
                    if l.starts_with("---") {
                        i += 1;
                        break;
                    }

                    if l.starts_with("**FRs covered:**") {
                        meta.frs_covered = l
                            .strip_prefix("**FRs covered:**")
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        i += 1;
                        continue;
                    }
                    if l.starts_with("**NFRs covered:**") {
                        meta.nfrs_covered = l
                            .strip_prefix("**NFRs covered:**")
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        i += 1;
                        continue;
                    }

                    if l.starts_with("### Story ") {
                        if let Some(story_key) = parse_story_heading_key(l, epic_num) {
                            let story_meta = parse_story_section(&lines, &mut i);
                            meta.stories.insert(story_key, story_meta);
                            continue;
                        }
                    }

                    i += 1;
                }

                metadata.insert(epic_num, meta);
                continue;
            }
        }

        i += 1;
    }
}

/// Parse "## Epic 1: Title" → 1, or "## Epic N: Title" → N
fn parse_epic_heading_number(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("## Epic ")?;
    let colon_pos = rest.find(':')?;
    rest[..colon_pos].trim().parse().ok()
}

/// Parse "### Story N.M: Title" → "N.M" key string
fn parse_story_heading_key(line: &str, _epic_num: u32) -> Option<String> {
    let rest = line.strip_prefix("### Story ")?;
    let colon_pos = rest.find(':')?;
    let num_part = rest[..colon_pos].trim();
    // Validate it looks like "N.M"
    if num_part.contains('.') {
        Some(num_part.to_string())
    } else {
        None
    }
}

/// Parse story section content (user story + acceptance criteria).
fn parse_story_section(lines: &[&str], i: &mut usize) -> StoryMetadata {
    let mut meta = StoryMetadata::default();
    *i += 1; // skip the heading line

    let mut user_story_lines = Vec::new();
    let mut ac_lines = Vec::new();
    let mut in_ac = false;

    while *i < lines.len() {
        let l = lines[*i];
        // Stop at next story or epic heading
        if l.starts_with("### Story ") || l.starts_with("## ") || l == "---" {
            break;
        }

        if l.starts_with("**Acceptance Criteria:**") {
            in_ac = true;
            *i += 1;
            continue;
        }

        if in_ac {
            if !l.trim().is_empty() {
                ac_lines.push(l.trim());
            }
        } else {
            // User story text (As a ..., I want ..., So that ...)
            let trimmed = l.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with("**")
                && !trimmed.starts_with("Dev Notes")
            {
                user_story_lines.push(trimmed);
            }
        }

        *i += 1;
    }

    meta.user_story = user_story_lines.join("\n");
    meta.acceptance_criteria = ac_lines.join("\n");
    meta
}

// ── Public query functions ───────────────────────────────────────────────────

/// Get full board data for the Kanban view.
pub fn get_board_data(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let (epics, project, last_updated) = parse_sprint_status(&config.base_dir)?;
    let md_metadata = parse_epics_markdown(&config.base_dir);

    let mut items: Vec<BoardItem> = Vec::new();

    // Build epic items
    for epic in &epics {
        let phase = epic_phase(epic.number);
        let done_count = epic.stories.iter().filter(|s| s.status == "done").count();
        let total = epic.stories.len();
        let pct = if total > 0 {
            (done_count as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        items.push(BoardItem {
            id: format!("epic-{}", epic.number),
            item_type: "epic".to_string(),
            title: epic.title.clone(),
            status: epic.status.clone(),
            phase,
            epic_id: format!("epic-{}", epic.number),
            epic_title: None,
            story_number: None,
            story_count: Some(total),
            stories_done: Some(done_count),
            progress_pct: Some((pct * 10.0).round() / 10.0),
        });

        // Build story items
        for story in &epic.stories {
            let story_num_key = &story.story_number; // "N.M"
            let enriched_title = md_metadata
                .get(&epic.number)
                .and_then(|m| m.stories.get(story_num_key))
                .map(|_| story.title.clone()) // title from slug is fine
                .unwrap_or_else(|| story.title.clone());

            items.push(BoardItem {
                id: story.id.clone(),
                item_type: "story".to_string(),
                title: enriched_title,
                status: story.status.clone(),
                phase,
                epic_id: format!("epic-{}", epic.number),
                epic_title: Some(epic.title.clone()),
                story_number: Some(story.story_number.clone()),
                story_count: None,
                stories_done: None,
                progress_pct: None,
            });
        }
    }

    // Compute summary
    let total_epics = epics.len();
    let total_stories: usize = epics.iter().map(|e| e.stories.len()).sum();
    let done_epics = epics.iter().filter(|e| e.status == "done").count();
    let done_stories = items
        .iter()
        .filter(|i| i.item_type == "story" && i.status == "done")
        .count();
    let in_progress = items
        .iter()
        .filter(|i| i.item_type == "story" && i.status == "in-progress")
        .count();
    let ready = items
        .iter()
        .filter(|i| i.item_type == "story" && i.status == "ready-for-dev")
        .count();
    let review = items
        .iter()
        .filter(|i| i.item_type == "story" && i.status == "review")
        .count();
    let backlog = items
        .iter()
        .filter(|i| i.item_type == "story" && i.status == "backlog")
        .count();
    let progress_pct = if total_stories > 0 {
        ((done_stories as f64 / total_stories as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let data = BoardData {
        project,
        last_updated,
        phases: vec![
            Phase {
                id: 1,
                label: phase_label(1).to_string(),
            },
            Phase {
                id: 2,
                label: phase_label(2).to_string(),
            },
            Phase {
                id: 3,
                label: phase_label(3).to_string(),
            },
        ],
        summary: BoardSummary {
            total_epics,
            total_stories,
            done_epics,
            done_stories,
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

/// Get available filter options.
pub fn get_filter_options(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let (epics, _project, _last_updated) = parse_sprint_status(&config.base_dir)?;

    let mut epic_options: Vec<FilterValue> = epics
        .iter()
        .map(|e| FilterValue {
            value: format!("epic-{}", e.number),
            label: format!("Epic {}: {}", e.number, e.title),
        })
        .collect();
    epic_options.sort_by(|a, b| a.value.cmp(&b.value));

    let options = FilterOptions {
        phases: vec![
            FilterValue {
                value: "1".to_string(),
                label: phase_label(1).to_string(),
            },
            FilterValue {
                value: "2".to_string(),
                label: phase_label(2).to_string(),
            },
            FilterValue {
                value: "3".to_string(),
                label: phase_label(3).to_string(),
            },
        ],
        epics: epic_options,
        statuses: vec![
            FilterValue {
                value: "backlog".to_string(),
                label: "Backlog".to_string(),
            },
            FilterValue {
                value: "ready-for-dev".to_string(),
                label: "Ready for Dev".to_string(),
            },
            FilterValue {
                value: "in-progress".to_string(),
                label: "In Progress".to_string(),
            },
            FilterValue {
                value: "review".to_string(),
                label: "Review".to_string(),
            },
            FilterValue {
                value: "done".to_string(),
                label: "Done".to_string(),
            },
        ],
        types: vec![
            FilterValue {
                value: "epic".to_string(),
                label: "Epic".to_string(),
            },
            FilterValue {
                value: "story".to_string(),
                label: "Story".to_string(),
            },
        ],
    };

    serde_json::to_value(&options)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

/// Get epic detail with stories.
pub fn get_epic_detail(
    epic_id: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let epic_num: u32 = epic_id
        .strip_prefix("epic-")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| WitPluginError::not_found(format!("Invalid epic ID: '{epic_id}'")))?;

    let (epics, _project, _last_updated) = parse_sprint_status(&config.base_dir)?;
    let md_metadata = parse_epics_markdown(&config.base_dir);

    let epic = epics
        .iter()
        .find(|e| e.number == epic_num)
        .ok_or_else(|| WitPluginError::not_found(format!("Epic '{epic_id}' not found")))?;

    let md = md_metadata.get(&epic_num);
    let done_count = epic.stories.iter().filter(|s| s.status == "done").count();
    let in_progress_count = epic
        .stories
        .iter()
        .filter(|s| s.status == "in-progress")
        .count();
    let phase = epic_phase(epic_num);

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

    let story_list: Vec<StoryListItem> = epic
        .stories
        .iter()
        .map(|s| StoryListItem {
            id: s.id.clone(),
            story_number: s.story_number.clone(),
            title: s.title.clone(),
            status: s.status.clone(),
        })
        .collect();

    let detail = EpicDetail {
        id: epic_id.to_string(),
        number: epic_num,
        title: epic.title.clone(),
        description: md.map(|m| m.description.clone()).unwrap_or_default(),
        status: epic.status.clone(),
        phase,
        phase_label: phase_label(phase).to_string(),
        frs_covered: md.map(|m| m.frs_covered.clone()).unwrap_or_default(),
        nfrs_covered: md.map(|m| m.nfrs_covered.clone()).unwrap_or_default(),
        story_count: epic.stories.len(),
        stories_done: done_count,
        stories_in_progress: in_progress_count,
        progress,
        story_list,
    };

    serde_json::to_value(&detail)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

/// Get story detail with acceptance criteria.
pub fn get_story_detail(
    story_id: &str,
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let (epics, _project, _last_updated) = parse_sprint_status(&config.base_dir)?;
    let md_metadata = parse_epics_markdown(&config.base_dir);

    // Find the story across all epics
    for epic in &epics {
        if let Some(story) = epic.stories.iter().find(|s| s.id == story_id) {
            let phase = epic_phase(epic.number);
            let md_story = md_metadata
                .get(&epic.number)
                .and_then(|m| m.stories.get(&story.story_number));

            let detail = StoryDetail {
                id: story.id.clone(),
                story_number: story.story_number.clone(),
                title: story.title.clone(),
                status: story.status.clone(),
                epic_id: format!("epic-{}", epic.number),
                epic_title: epic.title.clone(),
                phase,
                phase_label: phase_label(phase).to_string(),
                user_story: md_story.map(|m| m.user_story.clone()).unwrap_or_default(),
                acceptance_criteria: md_story
                    .map(|m| m.acceptance_criteria.clone())
                    .unwrap_or_default(),
            };

            return serde_json::to_value(&detail)
                .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")));
        }
    }

    Err(WitPluginError::not_found(format!(
        "Story '{story_id}' not found"
    )))
}

/// Get compact board summary for badge display.
pub fn get_board_summary(config: &WorkspaceConfig) -> Result<serde_json::Value, WitPluginError> {
    let (epics, _project, _last_updated) = parse_sprint_status(&config.base_dir)?;

    let total_stories: usize = epics.iter().map(|e| e.stories.len()).sum();
    let done_stories: usize = epics
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

    // Determine current active phase (highest phase with non-done epics)
    let current_phase = epics
        .iter()
        .filter(|e| e.status != "done")
        .map(|e| epic_phase(e.number))
        .max()
        .unwrap_or(3);

    let has_active_work = remaining == 0
        || epics.iter().any(|e| {
            e.stories
                .iter()
                .any(|s| s.status == "in-progress" || s.status == "review")
        });
    let sprint_progress = if has_active_work {
        "on-track"
    } else {
        "at-risk"
    };

    let summary = BoardSummaryCompact {
        sprint_progress: sprint_progress.to_string(),
        progress_pct,
        stories_remaining: remaining,
        current_phase: phase_label(current_phase).to_string(),
    };

    serde_json::to_value(&summary)
        .map_err(|e| WitPluginError::internal(format!("JSON serialization error: {e}")))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epic_phase_mapping() {
        assert_eq!(epic_phase(1), 1);
        assert_eq!(epic_phase(11), 1);
        assert_eq!(epic_phase(12), 2);
        assert_eq!(epic_phase(17), 2);
        assert_eq!(epic_phase(18), 3);
        assert_eq!(epic_phase(21), 3);
        assert_eq!(epic_phase(99), 0);
    }

    #[test]
    fn test_slug_to_title() {
        assert_eq!(
            slug_to_title("crate-scaffolding-and-process-manager"),
            "Crate Scaffolding and Process Manager"
        );
        assert_eq!(
            slug_to_title("implement-agentdefinitionprovider-trait-on-bmadagentregistry"),
            "Implement Agentdefinitionprovider Trait on Bmadagentregistry"
        );
        assert_eq!(slug_to_title(""), "");
    }

    #[test]
    fn test_parse_story_key() {
        let (epic, story, title) =
            parse_story_key("1-1-crate-scaffolding-and-process-manager").unwrap();
        assert_eq!(epic, 1);
        assert_eq!(story, 1);
        assert_eq!(title, "Crate Scaffolding and Process Manager");

        let (epic, story, title) =
            parse_story_key("12-3-add-unit-tests-for-agent-registry-completeness-and-correctness")
                .unwrap();
        assert_eq!(epic, 12);
        assert_eq!(story, 3);
        assert_eq!(
            title,
            "Add Unit Tests for Agent Registry Completeness and Correctness"
        );

        assert!(parse_story_key("epic-1").is_none());
        assert!(parse_story_key("not-a-story").is_none());
    }

    #[test]
    fn test_extract_epic_titles_from_comments() {
        let yaml = r#"
  # Epic 1: Claude Code Executor Plugin (claude-code-v2)
  epic-1: done
  # Epic 12: BMAD Agent Registry and Discovery
  epic-12: in-progress
"#;
        let titles = extract_epic_titles_from_comments(yaml);
        assert_eq!(
            titles.get(&1).unwrap(),
            "Claude Code Executor Plugin (claude-code-v2)"
        );
        assert_eq!(
            titles.get(&12).unwrap(),
            "BMAD Agent Registry and Discovery"
        );
    }

    #[test]
    fn test_parse_epic_heading_number() {
        assert_eq!(parse_epic_heading_number("## Epic 1: Title"), Some(1));
        assert_eq!(parse_epic_heading_number("## Epic 12: Title"), Some(12));
        assert_eq!(parse_epic_heading_number("## Epic List"), None);
        assert_eq!(parse_epic_heading_number("## Not Epic"), None);
    }

    #[test]
    fn test_get_board_data_from_real_files() {
        // This test reads the actual sprint-status.yaml in the repo
        let config = WorkspaceConfig::resolve(None);
        let result = get_board_data(&config);
        assert!(result.is_ok(), "get_board_data failed: {:?}", result.err());

        let data = result.unwrap();
        assert_eq!(data["project"], "bmad-method-flow");
        assert!(data["summary"]["total_epics"].as_u64().unwrap() >= 21);
        assert!(data["summary"]["total_stories"].as_u64().unwrap() > 0);
        assert!(data["items"].as_array().unwrap().len() > 21); // epics + stories
    }

    #[test]
    fn test_get_filter_options() {
        let config = WorkspaceConfig::resolve(None);
        let result = get_filter_options(&config);
        assert!(result.is_ok());

        let opts = result.unwrap();
        assert_eq!(opts["phases"].as_array().unwrap().len(), 3);
        assert!(opts["epics"].as_array().unwrap().len() >= 21);
        assert_eq!(opts["statuses"].as_array().unwrap().len(), 5);
        assert_eq!(opts["types"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_get_epic_detail() {
        let config = WorkspaceConfig::resolve(None);
        let result = get_epic_detail("epic-1", &config);
        assert!(result.is_ok());

        let detail = result.unwrap();
        assert_eq!(detail["id"], "epic-1");
        assert_eq!(detail["number"], 1);
        assert_eq!(detail["status"], "done");
        assert_eq!(detail["phase"], 1);
        assert!(detail["story_count"].as_u64().unwrap() >= 3);
    }

    #[test]
    fn test_get_epic_detail_not_found() {
        let config = WorkspaceConfig::resolve(None);
        let result = get_epic_detail("epic-999", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_story_detail() {
        let config = WorkspaceConfig::resolve(None);
        let result = get_story_detail("1-1-crate-scaffolding-and-process-manager", &config);
        assert!(result.is_ok());

        let detail = result.unwrap();
        assert_eq!(detail["epic_id"], "epic-1");
        assert_eq!(detail["status"], "done");
        assert_eq!(detail["phase"], 1);
    }

    #[test]
    fn test_get_story_detail_not_found() {
        let config = WorkspaceConfig::resolve(None);
        let result = get_story_detail("999-1-nonexistent", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_board_summary() {
        let config = WorkspaceConfig::resolve(None);
        let result = get_board_summary(&config);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert!(summary["progress_pct"].as_f64().unwrap() > 0.0);
        assert!(summary["stories_remaining"].as_u64().is_some());
    }
}
