use super::common::*;

#[test]
fn clean_reload_probe_rejects_dirty_buffer_before_apply() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    buffer.insert_at_cursor("local");
    app.buffers.push(buffer);

    assert!(!app.file_reload_targets_current_buffer(1, &path, version, false));
    assert!(app.file_reload_targets_current_buffer(
        1,
        &path,
        app.buffer(1).unwrap().version(),
        true
    ));
}

#[test]
fn forced_reload_of_dirty_buffer_with_same_text_marks_clean() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "same".to_owned());
    buffer.mark_dirty();
    let version = buffer.version();
    app.buffers.push(buffer);
    app.mark_buffer_changed_on_disk(1);
    let generation = app.external_change_generation;
    app.dirty_reload_buffer = Some(1);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, true);
    app.folding_ranges
        .insert(path.clone(), vec![folding_range(1, 2)]);
    app.folded_ranges.insert(
        path.clone(),
        vec![FoldedRange {
            start_line: 1,
            end_line: 2,
        }],
    );
    app.pending_fold_line = Some((path.clone(), 1));

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "same".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: true,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "same");
    assert!(!app.buffer(1).unwrap().is_dirty());
    assert!(!app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert_eq!(app.dirty_reload_buffer, None);
    assert!(!app.folding_ranges.contains_key(&path));
    assert!(!app.folded_ranges.contains_key(&path));
    assert!(app.pending_fold_line.is_none());
}

#[test]
fn forced_reload_result_preserves_newer_local_edits() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    buffer.mark_dirty();
    let version = buffer.version();
    buffer.insert_at_cursor("local-");
    app.buffers.push(buffer);
    app.mark_buffer_changed_on_disk(1);
    let generation = app.external_change_generation;
    app.dirty_reload_buffer = Some(1);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, true);
    app.folding_ranges
        .insert(path.clone(), vec![folding_range(1, 2)]);
    app.folded_ranges.insert(
        path.clone(),
        vec![FoldedRange {
            start_line: 1,
            end_line: 2,
        }],
    );
    app.pending_fold_line = Some((path.clone(), 1));

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "disk".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: true,
        lossy: false,
        binary: false,
    });

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "local-old");
    assert!(buffer.is_dirty());
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation);
    assert_eq!(app.dirty_reload_buffer, Some(1));
    assert!(app.status.starts_with("Skipped reload for "));
    assert!(app.status.ends_with(" because it changed locally"));
    assert_eq!(
        app.folding_ranges.get(&path).map(Vec::as_slice),
        Some(&[folding_range(1, 2)][..])
    );
    assert_eq!(
        app.folded_ranges.get(&path).map(Vec::as_slice),
        Some(
            &[FoldedRange {
                start_line: 1,
                end_line: 2,
            }][..]
        )
    );
    assert_eq!(app.pending_fold_line, Some((path, 1)));
}

#[test]
fn forced_reload_skipped_after_confirm_rearms_dirty_reload_guard() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.dirty_reload_buffer = Some(1);

    app.discard_and_reload_buffer_from_disk(1);

    assert_eq!(app.dirty_reload_buffer, None);
    let pending = app
        .in_flight_reloads
        .get(&1)
        .expect("forced reload should start")
        .clone();
    app.buffer_mut(1)
        .expect("buffer should exist")
        .insert_at_cursor("local-");

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: pending.request_id,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "disk".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version: pending.version,
        force_dirty: true,
        lossy: false,
        binary: false,
    });

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "local-old");
    assert!(buffer.is_dirty());
    assert_eq!(app.dirty_reload_buffer, Some(1));
    assert!(!app.buffer_changed_on_disk(1));
    assert!(app.status.starts_with("Skipped reload for "));
    assert!(app.status.ends_with(" because it changed locally"));
}

#[test]
fn clean_reload_preserves_manual_read_only_buffers() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    buffer.set_read_only(true);
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

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

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "new");
    assert!(buffer.is_read_only());
    assert!(!app.lossy_decoded_buffers.contains(&1));
    assert!(!app.binary_preview_buffers.contains(&1));
}

#[test]
fn clean_reload_preserves_manual_read_only_after_protected_preview_cycle() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    app.buffers.push(buffer);
    app.toggle_buffer_read_only(1);
    let version = app.buffer(1).expect("buffer exists").version();
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "preview".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: true,
    });

    assert!(app.buffer(1).is_some_and(TextBuffer::is_read_only));
    assert!(app.binary_preview_buffers.contains(&1));
    assert!(app.manual_read_only_buffers.contains(&1));

    let version = app.buffer(1).expect("buffer exists").version();
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path, "clean".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "clean");
    assert!(buffer.is_read_only());
    assert!(app.manual_read_only_buffers.contains(&1));
    assert!(!app.lossy_decoded_buffers.contains(&1));
    assert!(!app.binary_preview_buffers.contains(&1));
}

#[test]
fn clean_reload_clears_preview_only_read_only_buffers() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    buffer.set_read_only(true);
    let version = buffer.version();
    app.buffers.push(buffer);
    app.lossy_decoded_buffers.insert(1);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);

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

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "new");
    assert!(!buffer.is_read_only());
    assert!(!app.lossy_decoded_buffers.contains(&1));
    assert!(!app.binary_preview_buffers.contains(&1));
}

#[test]
fn protected_same_text_reload_invalidates_queued_static_diagnostics_request() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let buffer = TextBuffer::from_text(1, Some(path.clone()), "preview\n".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    app.diagnostics
        .replace_static(path.clone(), vec![static_diagnostic(&path)]);
    app.static_diagnostics_next_request_id = 1;
    app.static_diagnostics_active_request_ids.insert(1, 1);
    app.static_diagnostics_in_flight_request_ids.insert(1, 1);
    app.static_diagnostics_reload_queued.insert(1);

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path.clone(), "preview\n".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: true,
    });

    let buffer = app.buffer(1).unwrap();
    assert_eq!(buffer.text(), "preview\n");
    assert!(buffer.is_read_only());
    assert!(!app.lossy_decoded_buffers.contains(&1));
    assert!(app.binary_preview_buffers.contains(&1));
    assert!(app.diagnostics.for_path(&path).is_empty());
    assert_eq!(app.static_diagnostics_next_request_id, 2);
    assert_eq!(app.static_diagnostics_active_request_ids.get(&1), Some(&2));
    assert!(app.static_diagnostics_in_flight_request_ids.is_empty());
    assert!(app.static_diagnostics_reload_queued.is_empty());

    assert!(crate::ui_event_channel::send_ui_event(
        &app.tx,
        UiEvent::DiagnosticsComputed {
            request_id: 1,
            id: 1,
            path: path.clone(),
            version,
            diagnostics: vec![static_diagnostic(&path)],
        }
    ));

    assert_eq!(app.handle_events(), 1);
    assert!(app.diagnostics.for_path(&path).is_empty());
}

#[test]
fn clean_reload_clears_stale_folding_state_for_path() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let other_path = root.join("src/lib.rs");
    let mut app = app_for_test(root);

    let buffer = TextBuffer::from_text(
        1,
        Some(path.clone()),
        "fn main() {\n    old();\n}\n".to_owned(),
    );
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    app.folding_ranges
        .insert(path.clone(), vec![folding_range(1, 3)]);
    app.folding_ranges
        .insert(other_path.clone(), vec![folding_range(1, 2)]);
    app.folded_ranges.insert(
        path.clone(),
        vec![FoldedRange {
            start_line: 1,
            end_line: 3,
        }],
    );
    app.folded_ranges.insert(
        other_path.clone(),
        vec![FoldedRange {
            start_line: 1,
            end_line: 2,
        }],
    );
    app.pending_fold_line = Some((path.clone(), 1));

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(
            1,
            path.clone(),
            "fn main() {\n    new();\n}\n".to_owned(),
            ".".to_owned(),
        ),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(
        app.buffer(1).unwrap().text(),
        "fn main() {\n    new();\n}\n"
    );
    assert!(!app.folding_ranges.contains_key(&path));
    assert!(!app.folded_ranges.contains_key(&path));
    assert!(app.pending_fold_line.is_none());
    assert_eq!(
        app.folding_ranges.get(&other_path).map(Vec::as_slice),
        Some(&[folding_range(1, 2)][..])
    );
    assert_eq!(
        app.folded_ranges.get(&other_path).map(Vec::as_slice),
        Some(
            &[FoldedRange {
                start_line: 1,
                end_line: 2,
            }][..]
        )
    );
}

#[test]
fn clean_same_text_reload_preserves_folding_state() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let buffer = TextBuffer::from_text(
        1,
        Some(path.clone()),
        "fn main() {\n    same();\n}\n".to_owned(),
    );
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    app.folding_ranges
        .insert(path.clone(), vec![folding_range(1, 3)]);
    app.folded_ranges.insert(
        path.clone(),
        vec![FoldedRange {
            start_line: 1,
            end_line: 3,
        }],
    );
    app.pending_fold_line = Some((path.clone(), 1));
    app.status = "before".to_owned();

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(
            1,
            path.clone(),
            "fn main() {\n    same();\n}\n".to_owned(),
            ".".to_owned(),
        ),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.status, "before");
    assert_eq!(
        app.folding_ranges.get(&path).map(Vec::as_slice),
        Some(&[folding_range(1, 3)][..])
    );
    assert_eq!(
        app.folded_ranges.get(&path).map(Vec::as_slice),
        Some(
            &[FoldedRange {
                start_line: 1,
                end_line: 3,
            }][..]
        )
    );
    assert_eq!(app.pending_fold_line, Some((path, 1)));
}

#[test]
fn clean_reload_result_is_ignored_when_buffer_version_changed_after_request() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    buffer.insert_at_cursor("local-");
    buffer.mark_saved();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    app.status = "before".to_owned();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path, "disk".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert_eq!(app.buffer(1).unwrap().text(), "local-old");
    assert_eq!(app.status, "before");
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
}

#[test]
fn dirty_buffer_with_queued_clean_reload_marks_external_change_and_does_not_spawn() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
    app.queued_file_reloads.insert(
        1,
        QueuedFileReload {
            path: path.clone(),
            force_dirty: false,
        },
    );
    app.buffer_mut(1)
        .expect("buffer should exist")
        .insert_at_cursor("local-");
    app.buffer_mut(1).expect("buffer should exist").mark_dirty();
    let generation = app.external_change_generation;

    app.handle_file_reload_event(UiEvent::FileReloaded {
        root: app.workspace.root.clone(),
        generation: app.workspace_event_generation,
        request_id: 1,
        id: 1,
        path: path.clone(),
        buffer: loaded_text_buffer(1, path, "disk".to_owned(), ".".to_owned()),
        elapsed: Duration::ZERO,
        version,
        force_dirty: false,
        lossy: false,
        binary: false,
    });

    assert!(!app.in_flight_reloads.contains_key(&1));
    assert!(!app.queued_file_reloads.contains_key(&1));
    assert_eq!(app.external_change_generation, generation + 1);
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.buffer(1).unwrap().text(), "local-old");
}

#[test]
fn clean_reload_failure_is_ignored_when_buffer_version_changed_after_request() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);

    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "old".to_owned());
    let version = buffer.version();
    buffer.insert_at_cursor("local-");
    buffer.mark_saved();
    app.buffers.push(buffer);
    reserve_reload_for_test(&mut app, 1, path.clone(), version, false);
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

    assert_eq!(app.buffer(1).unwrap().text(), "local-old");
    assert_eq!(app.status, "before");
    assert!(app.buffer_changed_on_disk(1));
    assert_eq!(app.external_change_generation, generation + 1);
}
