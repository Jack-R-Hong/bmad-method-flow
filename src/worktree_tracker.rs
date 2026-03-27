use crate::workspace::WorkspaceConfig;
use pulse_plugin_sdk::error::WitPluginError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeStatus {
    Active,
    Completed,
    Failed,
    Orphaned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeEntry {
    pub worktree_path: String,
    pub branch_name: String,
    pub task_id: String,
    pub workflow_id: String,
    pub created_at: String,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorktreeRegistry {
    pub entries: Vec<WorktreeEntry>,
}

// ── Registry persistence ────────────────────────────────────────────────────

fn registry_path(config: &WorkspaceConfig) -> PathBuf {
    config.base_dir.join("config/worktree-registry.json")
}

pub fn load_registry(config: &WorkspaceConfig) -> Result<WorktreeRegistry, WitPluginError> {
    let path = registry_path(config);
    if !path.exists() {
        tracing::debug!(plugin = "coding-pack", "worktree registry file not found, returning empty");
        return Ok(WorktreeRegistry::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| WitPluginError::internal(format!("cannot read worktree registry: {e}")))?;
    let registry: WorktreeRegistry = serde_json::from_str(&content)
        .map_err(|e| WitPluginError::internal(format!("cannot parse worktree registry: {e}")))?;
    tracing::debug!(plugin = "coding-pack", entries = registry.entries.len(), "loaded worktree registry");
    Ok(registry)
}

fn save_registry(
    config: &WorkspaceConfig,
    registry: &WorktreeRegistry,
) -> Result<(), WitPluginError> {
    let path = registry_path(config);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WitPluginError::internal(format!("cannot create config dir: {e}")))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .map_err(|e| WitPluginError::internal(format!("JSON serialize error: {e}")))?;
    let tmp_path = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| WitPluginError::internal(format!("cannot create temp file: {e}")))?;
    std::io::Write::write_all(&mut file, json.as_bytes())
        .map_err(|e| WitPluginError::internal(format!("cannot write temp file: {e}")))?;
    file.sync_all()
        .map_err(|e| WitPluginError::internal(format!("cannot sync temp file: {e}")))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| WitPluginError::internal(format!("cannot rename temp to final: {e}")))?;
    Ok(())
}

// ── Registration and status updates ─────────────────────────────────────────

pub fn register_worktree(
    config: &WorkspaceConfig,
    worktree_path: &str,
    branch_name: &str,
    task_id: &str,
    workflow_id: &str,
) -> Result<(), WitPluginError> {
    let mut registry = load_registry(config)?;

    let now = now_iso8601();
    if let Some(existing) = registry
        .entries
        .iter_mut()
        .find(|e| e.worktree_path == worktree_path)
    {
        existing.branch_name = branch_name.to_string();
        existing.task_id = task_id.to_string();
        existing.workflow_id = workflow_id.to_string();
        existing.created_at = now;
        existing.status = WorktreeStatus::Active;
    } else {
        registry.entries.push(WorktreeEntry {
            worktree_path: worktree_path.to_string(),
            branch_name: branch_name.to_string(),
            task_id: task_id.to_string(),
            workflow_id: workflow_id.to_string(),
            created_at: now,
            status: WorktreeStatus::Active,
        });
    }

    save_registry(config, &registry)?;
    tracing::info!(plugin = "coding-pack", path = %worktree_path, task = %task_id, "registered worktree");
    Ok(())
}

pub fn update_worktree_status(
    config: &WorkspaceConfig,
    worktree_path: &str,
    new_status: WorktreeStatus,
) -> Result<(), WitPluginError> {
    let mut registry = load_registry(config)?;

    let entry = registry
        .entries
        .iter_mut()
        .find(|e| e.worktree_path == worktree_path)
        .ok_or_else(|| {
            WitPluginError::not_found(format!(
                "worktree entry not found for path: {}",
                worktree_path
            ))
        })?;

    entry.status = new_status.clone();
    save_registry(config, &registry)?;
    tracing::info!(plugin = "coding-pack", path = %worktree_path, status = ?new_status, "updated worktree status");
    Ok(())
}

// ── Timestamp helpers ───────────────────────────────────────────────────────

pub(crate) fn now_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    let mut year = 1970i32;
    let mut remaining_days = days as i32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let days_in_months: [i32; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &d in &days_in_months {
        if remaining_days < d {
            break;
        }
        remaining_days -= d;
        month += 1;
    }
    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

pub(crate) fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ── Cleanup types ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum CleanupOutcome {
    Removed,
    AlreadyGone,
    Error(String),
}

#[derive(Debug, Serialize)]
pub struct CleanupResult {
    pub removed: u32,
    pub skipped: u32,
    pub errors: u32,
    pub details: Vec<CleanupDetail>,
}

#[derive(Debug, Serialize)]
pub struct CleanupDetail {
    pub worktree_path: String,
    pub outcome: String,
}

// ── Cleanup functions ───────────────────────────────────────────────────────

fn remove_git_worktree(
    config: &WorkspaceConfig,
    worktree_path: &str,
) -> Result<(), WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["worktree", "remove", worktree_path])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git worktree remove: {e}")))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    // If the worktree path doesn't exist on disk, treat as already cleaned.
    // Also handle known error patterns (may be localized, so also check filesystem).
    if stderr.contains("is not a working tree")
        || stderr.contains("No such file or directory")
        || stderr.contains("is not a valid")
        || !std::path::Path::new(worktree_path).exists()
    {
        tracing::debug!(plugin = "coding-pack", path = worktree_path, "worktree already removed");
        return Ok(());
    }

    Err(WitPluginError::internal(format!(
        "git worktree remove failed for '{}': {}",
        worktree_path,
        stderr.trim()
    )))
}

fn delete_git_branch(
    config: &WorkspaceConfig,
    branch_name: &str,
) -> Result<(), WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["branch", "-d", branch_name])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git branch -d: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            plugin = "coding-pack",
            branch = branch_name,
            error = %stderr.trim(),
            "branch deletion failed — non-fatal"
        );
    }
    Ok(())
}

fn cleanup_single_worktree(config: &WorkspaceConfig, entry: &WorktreeEntry) -> CleanupOutcome {
    match remove_git_worktree(config, &entry.worktree_path) {
        Ok(()) => {
            // Worktree removed (or already gone) — try branch cleanup
            if let Err(e) = delete_git_branch(config, &entry.branch_name) {
                tracing::warn!(
                    plugin = "coding-pack",
                    branch = %entry.branch_name,
                    error = %e,
                    "branch deletion error during cleanup — non-fatal"
                );
            }
            CleanupOutcome::Removed
        }
        Err(e) => CleanupOutcome::Error(e.message),
    }
}

pub fn cleanup_completed_worktrees(
    config: &WorkspaceConfig,
) -> Result<CleanupResult, WitPluginError> {
    let registry = load_registry(config)?;
    let mut result = CleanupResult {
        removed: 0,
        skipped: 0,
        errors: 0,
        details: Vec::new(),
    };

    let mut kept_entries: Vec<WorktreeEntry> = Vec::new();

    for entry in &registry.entries {
        if entry.status != WorktreeStatus::Completed {
            // Skip Active entries; also skip Failed/Orphaned (handled by 24.3)
            if entry.status == WorktreeStatus::Active {
                result.skipped += 1;
                result.details.push(CleanupDetail {
                    worktree_path: entry.worktree_path.clone(),
                    outcome: "skipped_active".to_string(),
                });
            }
            kept_entries.push(entry.clone());
            continue;
        }

        // Completed — attempt cleanup
        match cleanup_single_worktree(config, entry) {
            CleanupOutcome::Removed | CleanupOutcome::AlreadyGone => {
                result.removed += 1;
                result.details.push(CleanupDetail {
                    worktree_path: entry.worktree_path.clone(),
                    outcome: "removed".to_string(),
                });
                // Do NOT keep this entry — it has been cleaned
            }
            CleanupOutcome::Error(msg) => {
                result.errors += 1;
                result.details.push(CleanupDetail {
                    worktree_path: entry.worktree_path.clone(),
                    outcome: format!("error: {msg}"),
                });
                kept_entries.push(entry.clone());
            }
        }
    }

    // Save updated registry with remaining entries
    let updated = WorktreeRegistry {
        entries: kept_entries,
    };
    save_registry(config, &updated)?;

    tracing::info!(
        plugin = "coding-pack",
        removed = result.removed,
        skipped = result.skipped,
        errors = result.errors,
        "worktree cleanup complete"
    );
    Ok(result)
}

// ── Recovery types ──────────────────────────────────────────────────────────

const ORPHAN_AGE_THRESHOLD_SECS: u64 = 3600; // 1 hour

#[derive(Debug, Serialize)]
pub struct RecoveryResult {
    pub pruned: bool,
    pub detected_untracked: u32,
    pub marked_orphaned: u32,
    pub cleaned: u32,
    pub skipped_recent: u32,
    pub errors: u32,
}

#[derive(Debug)]
pub struct GitWorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    pub head: String,
    pub bare: bool,
}

// ── Git worktree helpers ────────────────────────────────────────────────────

pub fn run_git_worktree_prune(config: &WorkspaceConfig) -> Result<(), WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git worktree prune: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(plugin = "coding-pack", error = %stderr.trim(), "git worktree prune failed — non-fatal");
    } else {
        tracing::debug!(plugin = "coding-pack", "git worktree prune complete");
    }
    Ok(())
}

pub fn list_git_worktrees(
    config: &WorkspaceConfig,
) -> Result<Vec<GitWorktreeInfo>, WitPluginError> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&config.base_dir)
        .output()
        .map_err(|e| WitPluginError::internal(format!("cannot run git worktree list: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WitPluginError::internal(format!(
            "git worktree list failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_porcelain_worktree_list(&stdout))
}

fn parse_porcelain_worktree_list(output: &str) -> Vec<GitWorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_head = String::new();
    let mut current_branch: Option<String> = None;
    let mut current_bare = false;

    for line in output.lines() {
        if line.is_empty() {
            if !current_path.is_empty() {
                worktrees.push(GitWorktreeInfo {
                    path: std::mem::take(&mut current_path),
                    head: std::mem::take(&mut current_head),
                    branch: current_branch.take(),
                    bare: current_bare,
                });
                current_bare = false;
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            current_head = head.to_string();
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            let branch_name = branch_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(branch_ref);
            current_branch = Some(branch_name.to_string());
        } else if line == "bare" {
            current_bare = true;
        }
    }
    // Handle last block (no trailing blank line)
    if !current_path.is_empty() {
        worktrees.push(GitWorktreeInfo {
            path: current_path,
            head: current_head,
            branch: current_branch,
            bare: current_bare,
        });
    }

    worktrees
}

// ── Untracked worktree detection ────────────────────────────────────────────

fn extract_ids_from_branch(branch: &str) -> Option<(String, String)> {
    let stripped = branch.strip_prefix("auto-dev/")?;
    let slash_pos = stripped.find('/')?;
    let workflow_id = stripped[..slash_pos].to_string();
    let task_id = stripped[slash_pos + 1..].to_string();
    if workflow_id.is_empty() || task_id.is_empty() {
        return None;
    }
    Some((workflow_id, task_id))
}

pub fn detect_untracked_worktrees(
    config: &WorkspaceConfig,
) -> Result<u32, WitPluginError> {
    let git_worktrees = list_git_worktrees(config)?;
    let mut registry = load_registry(config)?;
    let mut count = 0u32;

    for wt in &git_worktrees {
        // Skip bare repos and the main worktree
        if wt.bare {
            continue;
        }

        let branch = match &wt.branch {
            Some(b) => b,
            None => continue, // detached HEAD — skip
        };

        // Only consider auto-dev/ branches
        if !branch.starts_with("auto-dev/") {
            continue;
        }

        // Skip if main/master branch
        if branch == "main" || branch == "master" {
            continue;
        }

        // Check if already registered
        if registry.entries.iter().any(|e| e.worktree_path == wt.path) {
            continue;
        }

        // New untracked auto-dev worktree — register as orphaned
        let (workflow_id, task_id) = extract_ids_from_branch(branch)
            .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));

        registry.entries.push(WorktreeEntry {
            worktree_path: wt.path.clone(),
            branch_name: branch.clone(),
            task_id,
            workflow_id,
            created_at: now_iso8601(),
            status: WorktreeStatus::Orphaned,
        });
        count += 1;
    }

    if count > 0 {
        save_registry(config, &registry)?;
    }
    tracing::info!(plugin = "coding-pack", detected = count, "detected untracked auto-dev worktrees");
    Ok(count)
}

// ── Recovery pipeline ───────────────────────────────────────────────────────

pub fn recover_orphaned_worktrees(
    config: &WorkspaceConfig,
) -> Result<RecoveryResult, WitPluginError> {
    // Step 1: Prune git worktree metadata
    run_git_worktree_prune(config)?;

    // Step 2: Detect untracked worktrees
    let detected_untracked = detect_untracked_worktrees(config)?;

    // Step 3: Mark old failed entries as orphaned
    let mut registry = load_registry(config)?;
    let mut marked_orphaned = 0u32;
    let mut skipped_recent = 0u32;

    for entry in registry.entries.iter_mut() {
        if entry.status == WorktreeStatus::Failed {
            if is_old_enough_for_orphan(&entry.created_at) {
                entry.status = WorktreeStatus::Orphaned;
                marked_orphaned += 1;
            } else {
                skipped_recent += 1;
            }
        }
    }
    if marked_orphaned > 0 {
        save_registry(config, &registry)?;
    }

    // Step 4: Clean all orphaned entries
    // Reload registry (may have been updated in step 3)
    let registry = load_registry(config)?;
    let mut cleaned = 0u32;
    let mut errors = 0u32;
    let mut kept_entries: Vec<WorktreeEntry> = Vec::new();

    for entry in &registry.entries {
        if entry.status == WorktreeStatus::Orphaned {
            match cleanup_single_worktree(config, entry) {
                CleanupOutcome::Removed | CleanupOutcome::AlreadyGone => {
                    cleaned += 1;
                    // Don't keep — it was cleaned
                }
                CleanupOutcome::Error(_) => {
                    errors += 1;
                    kept_entries.push(entry.clone());
                }
            }
        } else {
            kept_entries.push(entry.clone());
        }
    }

    // Step 5: Save updated registry
    let updated = WorktreeRegistry {
        entries: kept_entries,
    };
    save_registry(config, &updated)?;

    let result = RecoveryResult {
        pruned: true,
        detected_untracked,
        marked_orphaned,
        cleaned,
        skipped_recent,
        errors,
    };

    tracing::info!(
        plugin = "coding-pack",
        cleaned = result.cleaned,
        errors = result.errors,
        "orphaned worktree recovery complete"
    );
    Ok(result)
}

fn is_old_enough_for_orphan(created_at: &str) -> bool {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match parse_iso8601_to_epoch(created_at) {
        Some(created_secs) => now_secs.saturating_sub(created_secs) > ORPHAN_AGE_THRESHOLD_SECS,
        None => true, // If we can't parse the timestamp, treat as old
    }
}

// ── Status reporting ────────────────────────────────────────────────────────

pub fn worktree_status(
    config: &WorkspaceConfig,
) -> Result<serde_json::Value, WitPluginError> {
    let registry = load_registry(config)?;

    let mut worktrees = Vec::new();
    let mut by_status = std::collections::BTreeMap::<String, u32>::new();

    for entry in &registry.entries {
        let status_str = serde_json::to_value(&entry.status)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| format!("{:?}", entry.status).to_lowercase());
        *by_status.entry(status_str.clone()).or_insert(0) += 1;

        let age = format_age(&entry.created_at);
        worktrees.push(serde_json::json!({
            "path": entry.worktree_path,
            "branch": entry.branch_name,
            "task_id": entry.task_id,
            "workflow_id": entry.workflow_id,
            "status": status_str,
            "age": age,
            "created_at": entry.created_at,
        }));
    }

    Ok(serde_json::json!({
        "worktrees": worktrees,
        "total": registry.entries.len(),
        "by_status": by_status,
    }))
}

// ── Age formatting helpers ──────────────────────────────────────────────────

pub(crate) fn parse_iso8601_to_epoch(ts: &str) -> Option<u64> {
    let parts: Vec<&str> = ts.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<u32> = time_str.split(':').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }
    let (year, month, day) = (date_parts[0] as i32, date_parts[1], date_parts[2]);
    let (hours, minutes, seconds) = (time_parts[0], time_parts[1], time_parts[2]);

    let mut total_days: i64 = 0;
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }
    let days_in_months: [u32; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    for dim in days_in_months.iter().take((month as usize).saturating_sub(1)) {
        total_days += *dim as i64;
    }
    total_days += (day.saturating_sub(1)) as i64;

    let total_secs =
        total_days * 86400 + hours as i64 * 3600 + minutes as i64 * 60 + seconds as i64;
    Some(total_secs as u64)
}

pub(crate) fn format_age(created_at: &str) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let created_secs = match parse_iso8601_to_epoch(created_at) {
        Some(s) => s,
        None => return "unknown".to_string(),
    };

    if now_secs < created_secs {
        return "0m".to_string();
    }

    let diff = now_secs - created_secs;
    let days = diff / 86400;
    let hours = (diff % 86400) / 3600;
    let minutes = (diff % 3600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(dir: &std::path::Path) -> WorkspaceConfig {
        WorkspaceConfig::from_base_dir(dir)
    }

    #[test]
    fn test_worktree_status_serialization() {
        let active = WorktreeStatus::Active;
        let json = serde_json::to_string(&active).unwrap();
        assert_eq!(json, r#""active""#);

        let completed = WorktreeStatus::Completed;
        let json = serde_json::to_string(&completed).unwrap();
        assert_eq!(json, r#""completed""#);

        let failed = WorktreeStatus::Failed;
        let json = serde_json::to_string(&failed).unwrap();
        assert_eq!(json, r#""failed""#);

        let orphaned = WorktreeStatus::Orphaned;
        let json = serde_json::to_string(&orphaned).unwrap();
        assert_eq!(json, r#""orphaned""#);

        // Round-trip
        let deserialized: WorktreeStatus = serde_json::from_str(r#""active""#).unwrap();
        assert_eq!(deserialized, WorktreeStatus::Active);
    }

    #[test]
    fn test_worktree_entry_serialization() {
        let entry = WorktreeEntry {
            worktree_path: "/tmp/wt/task-1".to_string(),
            branch_name: "auto-dev/coding-story-dev/task-1".to_string(),
            task_id: "task-1".to_string(),
            workflow_id: "coding-story-dev".to_string(),
            created_at: "2026-03-27T14:30:00Z".to_string(),
            status: WorktreeStatus::Active,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: WorktreeEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.worktree_path, entry.worktree_path);
        assert_eq!(deserialized.branch_name, entry.branch_name);
        assert_eq!(deserialized.task_id, entry.task_id);
        assert_eq!(deserialized.workflow_id, entry.workflow_id);
        assert_eq!(deserialized.created_at, entry.created_at);
        assert_eq!(deserialized.status, entry.status);
    }

    #[test]
    fn test_load_missing_registry_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        let registry = load_registry(&config).unwrap();
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn test_register_and_load_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        register_worktree(&config, "/tmp/wt/task-1", "auto-dev/coding/task-1", "task-1", "coding")
            .unwrap();
        let registry = load_registry(&config).unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].worktree_path, "/tmp/wt/task-1");
        assert_eq!(registry.entries[0].task_id, "task-1");
        assert_eq!(registry.entries[0].status, WorktreeStatus::Active);
    }

    #[test]
    fn test_register_duplicate_path_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        register_worktree(&config, "/tmp/wt/task-1", "branch-a", "task-1", "wf-a").unwrap();
        register_worktree(&config, "/tmp/wt/task-1", "branch-b", "task-2", "wf-b").unwrap();
        let registry = load_registry(&config).unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].task_id, "task-2");
        assert_eq!(registry.entries[0].workflow_id, "wf-b");
        assert_eq!(registry.entries[0].branch_name, "branch-b");
    }

    #[test]
    fn test_update_status_changes_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        register_worktree(&config, "/tmp/wt/task-1", "branch", "task-1", "wf").unwrap();
        update_worktree_status(&config, "/tmp/wt/task-1", WorktreeStatus::Completed).unwrap();
        let registry = load_registry(&config).unwrap();
        assert_eq!(registry.entries[0].status, WorktreeStatus::Completed);
    }

    #[test]
    fn test_update_status_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        let err =
            update_worktree_status(&config, "/nonexistent", WorktreeStatus::Completed).unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn test_atomic_write_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        let registry = WorktreeRegistry::default();
        save_registry(&config, &registry).unwrap();
        let path = registry_path(&config);
        assert!(path.exists());
        // No .tmp file should remain
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists());
    }

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        // Should match pattern YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));
    }

    // ── Story 24-2: Cleanup tests ───────────────────────────────────────

    /// Initialize a bare git repo in the temp dir so git commands work
    fn init_git_repo(dir: &std::path::Path) {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .expect("git init failed");
        // Create an initial commit so branches can be created
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .expect("git commit failed");
    }

    #[test]
    fn test_cleanup_completed_removes_entry() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        register_worktree(&config, "/nonexistent/wt/task-1", "branch-1", "task-1", "wf").unwrap();
        update_worktree_status(&config, "/nonexistent/wt/task-1", WorktreeStatus::Completed)
            .unwrap();
        let result = cleanup_completed_worktrees(&config).unwrap();
        assert_eq!(result.removed, 1);
        let registry = load_registry(&config).unwrap();
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn test_cleanup_skips_active() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        register_worktree(&config, "/wt/active", "b-active", "t-1", "wf").unwrap();
        register_worktree(&config, "/wt/completed", "b-completed", "t-2", "wf").unwrap();
        update_worktree_status(&config, "/wt/completed", WorktreeStatus::Completed).unwrap();
        let result = cleanup_completed_worktrees(&config).unwrap();
        assert_eq!(result.removed, 1);
        assert_eq!(result.skipped, 1);
        let registry = load_registry(&config).unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].worktree_path, "/wt/active");
        assert_eq!(registry.entries[0].status, WorktreeStatus::Active);
    }

    #[test]
    fn test_cleanup_result_counts() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        register_worktree(&config, "/wt/active1", "b1", "t1", "wf").unwrap();
        register_worktree(&config, "/wt/active2", "b2", "t2", "wf").unwrap();
        register_worktree(&config, "/wt/completed1", "b3", "t3", "wf").unwrap();
        register_worktree(&config, "/wt/completed2", "b4", "t4", "wf").unwrap();
        register_worktree(&config, "/wt/failed1", "b5", "t5", "wf").unwrap();
        update_worktree_status(&config, "/wt/completed1", WorktreeStatus::Completed).unwrap();
        update_worktree_status(&config, "/wt/completed2", WorktreeStatus::Completed).unwrap();
        update_worktree_status(&config, "/wt/failed1", WorktreeStatus::Failed).unwrap();
        let result = cleanup_completed_worktrees(&config).unwrap();
        assert_eq!(result.removed, 2);
        assert_eq!(result.skipped, 2); // 2 Active entries
        let registry = load_registry(&config).unwrap();
        // Active entries + Failed entry remain
        assert_eq!(registry.entries.len(), 3);
    }

    #[test]
    fn test_cleanup_nonexistent_path_still_removes_entry() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        register_worktree(
            &config,
            "/absolutely/nonexistent/worktree",
            "branch",
            "task",
            "wf",
        )
        .unwrap();
        update_worktree_status(
            &config,
            "/absolutely/nonexistent/worktree",
            WorktreeStatus::Completed,
        )
        .unwrap();
        let result = cleanup_completed_worktrees(&config).unwrap();
        // Should count as removed even though the path didn't exist on disk
        assert_eq!(result.removed, 1);
        let registry = load_registry(&config).unwrap();
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn test_remove_git_worktree_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        // Path doesn't exist on disk — should be treated as already gone
        let result = remove_git_worktree(&config, "/nonexistent/path");
        // Either Ok (treated as already gone) or Err — just shouldn't panic
        drop(result);
    }

    #[test]
    fn test_delete_git_branch_safe_delete() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        // Branch doesn't exist but should return Ok (non-fatal)
        let result = delete_git_branch(&config, "nonexistent-branch");
        assert!(result.is_ok());
    }

    // ── Story 24-3: Recovery and status tests ───────────────────────────

    #[test]
    fn test_parse_git_worktree_list_porcelain() {
        let output = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/main

worktree /home/user/project/.worktrees/auto-dev/coding-story-dev/task-42
HEAD def456abc789
branch refs/heads/auto-dev/coding-story-dev/task-42

worktree /home/user/project/.worktrees/feature/my-thing
HEAD 789abc123def
branch refs/heads/feature/my-thing
";
        let result = parse_porcelain_worktree_list(output);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].path, "/home/user/project");
        assert_eq!(result[0].branch.as_deref(), Some("main"));
        assert!(!result[0].bare);
        assert_eq!(
            result[1].path,
            "/home/user/project/.worktrees/auto-dev/coding-story-dev/task-42"
        );
        assert_eq!(
            result[1].branch.as_deref(),
            Some("auto-dev/coding-story-dev/task-42")
        );
        assert_eq!(result[2].branch.as_deref(), Some("feature/my-thing"));
    }

    #[test]
    fn test_detect_untracked_ignores_non_autodev_branches() {
        assert!(extract_ids_from_branch("main").is_none());
        assert!(extract_ids_from_branch("feature/foo").is_none());
        assert!(extract_ids_from_branch("bugfix/bar").is_none());
    }

    #[test]
    fn test_detect_untracked_adds_autodev_branches() {
        let result = extract_ids_from_branch("auto-dev/coding-story-dev/task-42");
        assert!(result.is_some());
        let (wf, task) = result.unwrap();
        assert_eq!(wf, "coding-story-dev");
        assert_eq!(task, "task-42");
    }

    #[test]
    fn test_extract_task_and_workflow_from_branch() {
        let (wf, task) =
            extract_ids_from_branch("auto-dev/coding-story-dev/task-42").unwrap();
        assert_eq!(wf, "coding-story-dev");
        assert_eq!(task, "task-42");

        let (wf, task) =
            extract_ids_from_branch("auto-dev/coding-bug-fix/task-99").unwrap();
        assert_eq!(wf, "coding-bug-fix");
        assert_eq!(task, "task-99");

        // Edge: no task part
        assert!(extract_ids_from_branch("auto-dev/").is_none());
        // Edge: no workflow part
        assert!(extract_ids_from_branch("auto-dev//task").is_none());
    }

    #[test]
    fn test_recover_marks_old_failed_as_orphaned() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        register_worktree(&config, "/wt/old-failed", "auto-dev/wf/task", "task", "wf").unwrap();
        let mut registry = load_registry(&config).unwrap();
        registry.entries[0].status = WorktreeStatus::Failed;
        registry.entries[0].created_at = "2020-01-01T00:00:00Z".to_string();
        save_registry(&config, &registry).unwrap();

        let result = recover_orphaned_worktrees(&config).unwrap();
        assert_eq!(result.marked_orphaned, 1);
        assert_eq!(result.cleaned, 1);
    }

    #[test]
    fn test_recover_skips_recent_failed() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let config = temp_config(tmp.path());
        register_worktree(&config, "/wt/recent-failed", "branch", "task", "wf").unwrap();
        let mut registry = load_registry(&config).unwrap();
        registry.entries[0].status = WorktreeStatus::Failed;
        save_registry(&config, &registry).unwrap();

        let result = recover_orphaned_worktrees(&config).unwrap();
        assert_eq!(result.skipped_recent, 1);
        assert_eq!(result.marked_orphaned, 0);
        let registry = load_registry(&config).unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].status, WorktreeStatus::Failed);
    }

    #[test]
    fn test_format_age_hours_minutes() {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let target = now_secs - (2 * 3600 + 15 * 60);
        let ts = epoch_to_iso8601(target);
        let age = format_age(&ts);
        assert!(age.starts_with("2h"), "expected '2h Xm', got '{}'", age);
        assert!(age.contains('m'), "expected minutes, got '{}'", age);
    }

    #[test]
    fn test_format_age_minutes_only() {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let target = now_secs - (5 * 60);
        let ts = epoch_to_iso8601(target);
        let age = format_age(&ts);
        assert_eq!(age, "5m");
    }

    #[test]
    fn test_format_age_days() {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let target = now_secs - (3 * 86400 + 1 * 3600);
        let ts = epoch_to_iso8601(target);
        let age = format_age(&ts);
        assert!(age.starts_with("3d"), "expected '3d Xh', got '{}'", age);
    }

    #[test]
    fn test_format_age_unknown_timestamp() {
        assert_eq!(format_age("not-a-timestamp"), "unknown");
    }

    #[test]
    fn test_worktree_status_returns_json() {
        let tmp = tempfile::tempdir().unwrap();
        let config = temp_config(tmp.path());
        register_worktree(&config, "/wt/task-1", "auto-dev/wf/task-1", "task-1", "wf").unwrap();
        register_worktree(&config, "/wt/task-2", "auto-dev/wf/task-2", "task-2", "wf").unwrap();
        update_worktree_status(&config, "/wt/task-2", WorktreeStatus::Completed).unwrap();
        let status = worktree_status(&config).unwrap();
        assert_eq!(status["total"], 2);
        assert!(status["worktrees"].is_array());
        assert_eq!(status["worktrees"].as_array().unwrap().len(), 2);
        assert!(status["by_status"].is_object());
        assert_eq!(status["by_status"]["active"], 1);
        assert_eq!(status["by_status"]["completed"], 1);
    }

    #[test]
    fn test_parse_iso8601_to_epoch_roundtrip() {
        let epoch = parse_iso8601_to_epoch("1970-01-01T00:00:00Z");
        assert_eq!(epoch, Some(0));

        let epoch = parse_iso8601_to_epoch("2026-03-27T14:30:00Z");
        assert!(epoch.is_some());
        assert!(epoch.unwrap() > 1_700_000_000);
    }

    #[test]
    fn test_parse_iso8601_invalid() {
        assert!(parse_iso8601_to_epoch("not-a-date").is_none());
        assert!(parse_iso8601_to_epoch("").is_none());
        assert!(parse_iso8601_to_epoch("2026-03-27").is_none());
    }

    /// Helper: convert epoch seconds back to ISO 8601 for test fixtures
    fn epoch_to_iso8601(secs: u64) -> String {
        let days = secs / 86400;
        let time_secs = secs % 86400;
        let hours = time_secs / 3600;
        let minutes = (time_secs % 3600) / 60;
        let seconds = time_secs % 60;
        let mut year = 1970i32;
        let mut remaining_days = days as i32;
        loop {
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }
        let days_in_months: [i32; 12] = if is_leap_year(year) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        let mut month = 1;
        for &d in &days_in_months {
            if remaining_days < d {
                break;
            }
            remaining_days -= d;
            month += 1;
        }
        let day = remaining_days + 1;
        format!(
            "{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z"
        )
    }
}
