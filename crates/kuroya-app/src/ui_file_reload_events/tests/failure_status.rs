use super::common::*;

#[test]
fn current_file_reload_failed_completion_spawns_queued_reload_once() {
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

    assert!(!app.queued_file_reloads.contains_key(&1));
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
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
fn clean_reload_failure_marks_same_path_external_change() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root.clone());

    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
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

    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}

#[test]
fn clean_reload_failure_marks_equivalent_path_external_change() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let equivalent_path = root.join("src").join(".").join("main.rs");
    let mut app = app_for_test(root.clone());

    let buffer = TextBuffer::from_text(1, Some(path), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, equivalent_path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: equivalent_path,
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}

#[test]
fn reload_failure_status_sanitizes_path_and_error_text() {
    let root = PathBuf::from("workspace");
    let path = root.join(format!("bad\n{}\u{202e}.rs", "very-long-name-".repeat(16)));
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path,
        error: format!("denied\nbecause \u{202e}{}", "x".repeat(256)),
        version,
        force_dirty: false,
    });

    assert!(app.status.starts_with("Could not reload "));
    assert!(!app.status.contains('\n'));
    assert!(!app.status.contains('\u{202e}'));
}
