use super::common::*;

#[test]
fn equivalent_root_file_reloaded_event_is_applied() {
    let root = PathBuf::from("workspace");
    let event_root = root.join("src").join("..");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: event_root,
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
}

#[test]
fn equivalent_root_file_reload_failed_event_is_applied() {
    let root = PathBuf::from("workspace");
    let event_root = root.join("src").join("..");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloadFailed {
        root: event_root,
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path,
        error: "denied".to_owned(),
        version,
        force_dirty: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}

#[test]
fn equivalent_path_file_reloaded_event_is_applied() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let equivalent_path = root.join("src").join(".").join("main.rs");
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
        path: equivalent_path.clone(),
        buffer: loaded_text_buffer(1, equivalent_path, "new".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "new");
}

#[test]
fn equivalent_path_file_reload_failed_event_is_applied() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let equivalent_path = root.join("src").join(".").join("main.rs");
    let mut app = app_for_test(root);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path, version, false);
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

    assert_eq!(app.buffer(1).unwrap().text(), "old");
    assert!(!app.in_flight_reloads.contains_key(&1));
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.status.starts_with("Could not reload "));
}
