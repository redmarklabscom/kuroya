use super::common::*;

#[test]
fn file_reload_events_from_other_workspace_are_ignored() {
    let root = PathBuf::from("workspace");
    let other_root = PathBuf::from("other-workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(path.clone()),
        "old".to_owned(),
    ));
    let version = app.buffer(1).unwrap().version();
    app.status = "before".to_owned();

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: other_root,
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "new".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
}

#[test]
fn stale_file_reloaded_event_is_ignored_after_workspace_reset() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root.clone());
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(path.clone()),
        "old".to_owned(),
    ));
    let version = app.buffer(1).unwrap().version();
    let stale_generation = app.workspace_event_generation;
    app.status = "before".to_owned();
    app.reset_open_workspace_state();

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root,
        generation: stale_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "new".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert!(app.buffers.is_empty());
    assert_eq!(app.status, "before");
    assert!(app.in_flight_reloads.is_empty());
    assert!(app.queued_file_reloads.is_empty());
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
    assert_eq!(app.changed_on_disk_buffer_count(), 0);
    assert_eq!(app.dirty_reload_buffer, None);
}

#[test]
fn stale_file_reload_failed_event_is_ignored_after_workspace_reset() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root.clone());
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(path.clone()),
        "old".to_owned(),
    ));
    let version = app.buffer(1).unwrap().version();
    let stale_generation = app.workspace_event_generation;
    app.status = "before".to_owned();
    app.reset_open_workspace_state();

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root,
        generation: stale_generation,
        request_id: 1,
        id: 1,
        path,
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert!(app.buffers.is_empty());
    assert_eq!(app.status, "before");
    assert!(app.in_flight_reloads.is_empty());
    assert!(app.queued_file_reloads.is_empty());
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
    assert_eq!(app.changed_on_disk_buffer_count(), 0);
    assert_eq!(app.dirty_reload_buffer, None);
}

#[test]
fn stale_workspace_file_reloaded_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root.clone());
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 1, path.clone(), version, false);
    app.queued_file_reloads.insert(
        1,
        QueuedFileReload {
            path: queued_path.clone(),
            force_dirty: true,
        },
    );
    let stale_generation = app.workspace_event_generation.wrapping_sub(1);
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root,
        generation: stale_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "new".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
    assert_in_flight_reload_for_test(&app, 1, 1, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn stale_workspace_file_reload_failed_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root.clone());
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 1, path.clone(), version, false);
    app.queued_file_reloads.insert(
        1,
        QueuedFileReload {
            path: queued_path.clone(),
            force_dirty: true,
        },
    );
    let stale_generation = app.workspace_event_generation.wrapping_sub(1);
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root,
        generation: stale_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
    assert_in_flight_reload_for_test(&app, 1, 1, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn untracked_file_reloaded_event_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(path.clone()),
        "old".to_owned(),
    ));
    let version = app.buffer(1).unwrap().version();
    app.mark_buffer_changed_on_disk(1);
    app.dirty_reload_buffer = Some(1);
    app.folding_ranges
        .insert(path.clone(), vec![folding_range(1, 2)]);
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "new".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: true,
        binary: true,
    });

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "old");
    assert!(!buffer.is_read_only());
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.dirty_reload_buffer, Some(1));
    assert!(!app.lossy_decoded_buffers.contains(&1));
    assert!(!app.binary_preview_buffers.contains(&1));
    assert_eq!(
        app.folding_ranges.get(&path).map(Vec::as_slice),
        Some(&[folding_range(1, 2)][..])
    );
}

#[test]
fn untracked_file_reload_failed_event_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(path.clone()),
        "old".to_owned(),
    ));
    let version = app.buffer(1).unwrap().version();
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path,
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
}

#[test]
fn untracked_file_reloaded_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    app.queued_file_reloads.insert(
        1,
        QueuedFileReload {
            path: queued_path.clone(),
            force_dirty: true,
        },
    );
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path, "new".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(app.in_flight_reloads.is_empty());
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(!app.buffer_changed_on_disk(1));
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn untracked_file_reload_failed_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    app.queued_file_reloads.insert(
        1,
        QueuedFileReload {
            path: queued_path.clone(),
            force_dirty: true,
        },
    );
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path,
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(app.in_flight_reloads.is_empty());
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(!app.buffer_changed_on_disk(1));
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}
