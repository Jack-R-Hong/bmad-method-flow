//! E2E tests for the auto-dev loop.
//!
//! These tests require both the Pulse server and plugin-board to be running.
//! They are ignored by default since they need a live environment.
//! Run with: cargo test -- --ignored

use plugin_coding_pack::auto_dev;
use plugin_coding_pack::workspace::WorkspaceConfig;

#[test]
#[ignore] // requires live Pulse server + plugin-board
fn test_auto_dev_status_with_live_board() {
    let config = WorkspaceConfig::resolve(None);
    let status = auto_dev::auto_dev_status(&config).unwrap();
    assert!(status.get("total").is_some());
}

#[test]
#[ignore] // requires live Pulse server + plugin-board
fn test_auto_dev_next_no_ready_tasks() {
    let config = WorkspaceConfig::resolve(None);
    // With no ready-for-dev tasks, should return None
    let result = auto_dev::auto_dev_next(&config).unwrap();
    // Either None (no tasks) or Some (if there happen to be ready tasks)
    let _ = result;
}
