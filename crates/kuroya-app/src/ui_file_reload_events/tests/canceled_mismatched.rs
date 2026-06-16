use super::common::*;

#[test]
fn duplicate_file_reloaded_completion_is_ignored_after_current_completion() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

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
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new");
    let status = app.status.clone();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path,
        buffer: loaded_text_buffer(
            1,
            PathBuf::from("workspace/src/main.rs"),
            "duplicate".to_owned(),
            ".".to_owned(),
        ),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new");
    assert_eq!(app.status, status);
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
}

#[test]
fn current_file_reloaded_rejects_mismatched_payload_buffer_path() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let other_path = root.join("src/lib.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, other_path, "wrong".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: true,
        binary: true,
    });

    let buffer = app.buffer(1).expect("current buffer should remain open");
    assert_eq!(buffer.text(), "old");
    assert!(!buffer.is_read_only());
    assert!(app.in_flight_reloads.is_empty());
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(!app.lossy_decoded_buffers.contains(&1));
    assert!(!app.binary_preview_buffers.contains(&1));
    assert_eq!(
        app.status,
        "Could not reload main.rs: loaded buffer path did not match request"
    );
}

#[test]
fn current_file_reloaded_completion_spawns_queued_reload_once() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src").join(".").join("main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    app.file_reload_next_request_id = 1;
    app.queued_file_reloads.insert(
        1,
        QueuedFileReload {
            path: queued_path.clone(),
            force_dirty: false,
        },
    );
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

    assert!(!app.queued_file_reloads.contains_key(&1));
    assert!(app.status.starts_with("Reloaded "));
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
    assert_eq!(
        app.in_flight_reloads.get(&1),
        Some(&PendingFileReload {
            request_id: 2,
            path: queued_path,
            version: app.buffer(1).expect("buffer should exist").version(),
            force_dirty: false,
        })
    );
}

#[test]
fn canceled_file_reloaded_completion_is_ignored_and_tombstone_removed() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 1,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path,
        buffer: loaded_text_buffer(
            1,
            PathBuf::from("workspace/src/main.rs"),
            "new".to_owned(),
            ".".to_owned(),
        ),
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
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn canceled_file_reload_failed_completion_is_ignored_and_tombstone_removed() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 1, path.clone(), version, false);
    app.cancel_deferred_reload_work(1);
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
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn stale_file_reloaded_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 10,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
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
        request_id: 10,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "old-canceled".to_owned(), ".".to_owned()),
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
    assert_in_flight_reload_for_test(&app, 1, 11, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn stale_file_reload_failed_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 10,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
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
        request_id: 10,
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
    assert_in_flight_reload_for_test(&app, 1, 11, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn stale_in_flight_file_reloaded_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
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
        request_id: 10,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "stale".to_owned(), ".".to_owned()),
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
    assert_in_flight_reload_for_test(&app, 1, 11, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn stale_in_flight_file_reload_failed_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
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
        request_id: 10,
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
    assert_in_flight_reload_for_test(&app, 1, 11, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn mismatched_reload_key_file_reloaded_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
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
        request_id: 11,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "mismatch".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: true,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
    assert_in_flight_reload_for_test(&app, 1, 11, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn mismatched_reload_key_file_reload_failed_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
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
        request_id: 11,
        id: 1,
        path: path.clone(),
        error: "denied".to_owned(),
        version,
        force_dirty: true,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
    assert_in_flight_reload_for_test(&app, 1, 11, &path, version, false);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());
}

#[test]
fn mismatched_canceled_reload_key_file_reloaded_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version,
            force_dirty: true,
        },
    );
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
        request_id: 11,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "stale".to_owned(), ".".to_owned()),
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
    assert_only_canceled_reload_present_for_test(&app, 1, 11, &path, version, true);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
}

#[test]
fn mismatched_canceled_reload_key_file_reload_failed_completion_does_not_drain_queued_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let queued_path = root.join("src/queued.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version,
            force_dirty: true,
        },
    );
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
        request_id: 11,
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
    assert_only_canceled_reload_present_for_test(&app, 1, 11, &path, version, true);
    assert_queued_reload_for_test(&app, 1, &queued_path, true);
}

#[test]
fn current_reload_with_same_request_as_mismatched_canceled_tombstone_applies() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version,
            force_dirty: true,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 11,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "new-valid".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new-valid");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 1, 11, &path, version, true);
    assert!(!app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation);
    assert!(app.status.starts_with("Reloaded "));
}

#[test]
fn current_reload_failure_with_same_request_as_mismatched_canceled_tombstone_applies() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version,
            force_dirty: true,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 11,
        id: 1,
        path: path.clone(),
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 1, 11, &path, version, true);
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}

#[test]
fn current_reload_with_same_tuple_as_canceled_old_request_applies() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 10,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 11,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "new-valid".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new-valid");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 1, 10, &path, version, false);
    assert!(!app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation);
    assert!(app.status.starts_with("Reloaded "));
}

#[test]
fn current_reload_failure_with_same_tuple_as_canceled_old_request_applies() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 10,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 11,
        id: 1,
        path: path.clone(),
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 1, 10, &path, version, false);
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}

#[test]
fn current_reload_failure_ignores_unrelated_canceled_tombstone_for_same_buffer() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let canceled_path = root.join("src/other.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 10,
            path: canceled_path.clone(),
            version,
            force_dirty: false,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 11,
        id: 1,
        path,
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 1, 10, &canceled_path, version, false);
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}

#[test]
fn old_canceled_reload_completion_does_not_remove_later_matching_request() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    insert_canceled_reload_for_test(
        &mut app,
        1,
        PendingFileReload {
            request_id: 10,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );
    insert_in_flight_reload_for_test(&mut app, 1, 11, path.clone(), version, false);
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 10,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(
            1,
            path.clone(),
            "old-canceled-result".to_owned(),
            ".".to_owned(),
        ),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert_eq!(app.status, "before");
    assert_eq!(app.external_change_generation, generation);
    assert_eq!(
        app.in_flight_reloads
            .get(&1)
            .map(|pending| pending.request_id),
        Some(11)
    );
    assert!(app.canceled_file_reloads.is_empty());
    assert!(app.canceled_file_reload_order.is_empty());

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 11,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path, "new-valid".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new-valid");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert!(!app.buffer_changed_on_disk(1));
}

#[test]
fn canceled_reload_for_other_buffer_does_not_stale_current_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    let pending = PendingFileReload {
        request_id: 1,
        path: path.clone(),
        version,
        force_dirty: false,
    };
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(
        &mut app,
        1,
        pending.request_id,
        pending.path.clone(),
        pending.version,
        pending.force_dirty,
    );
    insert_canceled_reload_for_test(&mut app, 2, pending);
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
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 2, 1, &path, version, false);
    assert_eq!(app.external_change_generation, generation);
    assert!(!app.buffer_changed_on_disk(1));
    assert!(app.status.starts_with("Reloaded "));
}

#[test]
fn canceled_reload_for_other_buffer_does_not_stale_current_reload_failure() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    let pending = PendingFileReload {
        request_id: 1,
        path: path.clone(),
        version,
        force_dirty: false,
    };
    app.buffers.push(buffer);
    insert_in_flight_reload_for_test(
        &mut app,
        1,
        pending.request_id,
        pending.path.clone(),
        pending.version,
        pending.force_dirty,
    );
    insert_canceled_reload_for_test(&mut app, 2, pending);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert_only_canceled_reload_present_for_test(&app, 2, 1, &path, version, false);
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}
