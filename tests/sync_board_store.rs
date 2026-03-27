//! One-shot utility: sync board store from sprint-status.yaml and add auto-dev assignments.
//! Run with: cargo test --test sync_board_store -- --nocapture --ignored

#[test]
#[ignore]
fn sync_board_and_add_assignments() {
    let config = plugin_coding_pack::workspace::WorkspaceConfig::resolve(None);

    // 1. Sync epics/stories from sprint-status.yaml
    let mut store = plugin_coding_pack::board_store::sync_from_artifacts(&config)
        .expect("sync should succeed");
    println!("Synced {} epics from sprint-status.yaml", store.epics.len());

    // 2. Add auto-dev task assignments
    store.assignments = vec![
        plugin_coding_pack::board_store::StoreAssignment {
            id: "task-1-autodev-status".to_string(),
            title: "Add auto-dev-status action".to_string(),
            status: "done".to_string(),
            description: "Added auto_dev_status() function and pack action".to_string(),
            assignee: "auto-dev".to_string(),
            priority: "high".to_string(),
            labels: vec!["quick".to_string()],
            tasks: vec![
                plugin_coding_pack::board_store::SubTask { id: "st-1".into(), title: "Implement auto_dev_status()".into(), done: true },
                plugin_coding_pack::board_store::SubTask { id: "st-2".into(), title: "Register in pack.rs".into(), done: true },
                plugin_coding_pack::board_store::SubTask { id: "st-3".into(), title: "Add unit tests".into(), done: true },
            ],
            comments: vec![
                plugin_coding_pack::board_store::Comment { id: "c1".into(), author: "auto-dev".into(), content: "[auto-dev] Completed. All tests passing.".into(), timestamp: "2026-03-27".into() },
            ],
            workflow_id: String::new(),
        },
        plugin_coding_pack::board_store::StoreAssignment {
            id: "task-2-retry-logic".to_string(),
            title: "Add auto-dev retry on test failure".to_string(),
            status: "ready-for-dev".to_string(),
            description: "When auto_dev_next detects test failure and max_retries > 0, re-invoke the workflow with failure context appended.".to_string(),
            assignee: String::new(),
            priority: "high".to_string(),
            labels: vec!["feature".to_string()],
            tasks: vec![
                plugin_coding_pack::board_store::SubTask { id: "st-1".into(), title: "Add retry loop to auto_dev_next".into(), done: false },
                plugin_coding_pack::board_store::SubTask { id: "st-2".into(), title: "Append test failure output to retry input".into(), done: false },
                plugin_coding_pack::board_store::SubTask { id: "st-3".into(), title: "Add unit test for retry".into(), done: false },
            ],
            comments: vec![],
            workflow_id: String::new(),
        },
        plugin_coding_pack::board_store::StoreAssignment {
            id: "task-3-watch-interval".to_string(),
            title: "Add poll interval to auto-dev-watch".to_string(),
            status: "backlog".to_string(),
            description: "Add optional poll_interval_secs config for watch mode.".to_string(),
            assignee: String::new(),
            priority: "medium".to_string(),
            labels: vec!["quick".to_string()],
            tasks: vec![],
            comments: vec![],
            workflow_id: String::new(),
        },
    ];

    plugin_coding_pack::board_store::save_store(&config.base_dir, &store)
        .expect("save should succeed");

    println!("Board store saved with {} epics and {} assignments",
        store.epics.len(), store.assignments.len());
    println!("File: {:?}", plugin_coding_pack::board_store::store_path(&config.base_dir));
}
