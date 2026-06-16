use super::common::*;

#[test]
fn equivalent_root_workspace_apply_edit_updates_open_buffer() {
    let root = PathBuf::from("workspace");
    let event_root = root.join("src").join("..");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path.clone()),
        "fn main() {}\n".to_owned(),
    ));

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: event_root,
        generation: 1,
        request_id: LspRequestId::Number(18),
        label: Some("Apply equivalent root edit".to_owned()),
        edits: Some(vec![edit(&path, "mod changed;\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert_eq!(
        app.buffer(7).expect("buffer").text(),
        "mod changed;\nfn main() {}\n"
    );
}

#[test]
fn workspace_apply_edit_versioned_equivalent_path_updates_open_buffer() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let edit_path = root.join("src").join("..").join("src").join("main.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path),
        "fn main() {}\n".to_owned(),
    ));
    let version = app.buffer(7).expect("buffer").version();

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(21),
        label: Some("Apply equivalent path edit".to_owned()),
        edits: Some(vec![edit(&edit_path, "mod changed;\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::from([(edit_path, buffer_lsp_version(version))]),
        error: None,
    });

    assert_eq!(
        app.buffer(7).expect("buffer").text(),
        "mod changed;\nfn main() {}\n"
    );
}

#[test]
fn workspace_apply_edit_equivalent_path_keeps_open_buffer_safeguards() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let edit_path = root.join("src").join("..").join("src").join("main.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path),
        "fn main() {}\n".to_owned(),
    ));
    app.binary_preview_buffers.insert(7);

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(22),
        label: Some("Apply unsafe equivalent path edit".to_owned()),
        edits: Some(vec![edit(&edit_path, "mod changed;\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
    assert!(
        app.status.contains("unsafe open buffer skipped"),
        "{}",
        app.status
    );
}

#[test]
fn workspace_apply_edit_rejects_dirty_buffer_with_pending_clean_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    let version = buffer.version();
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.in_flight_reloads.insert(
        7,
        PendingFileReload {
            request_id: 1,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(23),
        label: Some("Apply pending reload edit".to_owned()),
        edits: Some(vec![edit(&path, "changed\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
    assert!(
        app.status.contains("unsafe open buffer skipped"),
        "{}",
        app.status
    );
    assert!(app.status.contains("changed on disk"), "{}", app.status);
}

#[test]
fn workspace_apply_edit_rejects_dirty_buffer_with_queued_clean_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.queued_file_reloads.insert(
        7,
        QueuedFileReload {
            path: path.clone(),
            force_dirty: false,
        },
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(24),
        label: Some("Apply queued reload edit".to_owned()),
        edits: Some(vec![edit(&path, "changed\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
    assert!(
        app.status.contains("unsafe open buffer skipped"),
        "{}",
        app.status
    );
    assert!(app.status.contains("changed on disk"), "{}", app.status);
}

#[test]
fn workspace_apply_edit_rejects_large_file_mode_open_buffer() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/large.rs");
    let large_text = std::iter::repeat_n("x", LARGE_FILE_MODE_MAX_LINES + 1)
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path.clone()),
        large_text.clone(),
    ));

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(47),
        label: Some("Apply large file edit".to_owned()),
        edits: Some(vec![edit(&path, "changed\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert_eq!(app.buffer(7).expect("buffer").text(), large_text);
    assert!(
        app.status.contains("unsafe open buffer skipped"),
        "{}",
        app.status
    );
    assert!(app.status.contains("large.rs"), "{}", app.status);
    assert!(app.status.contains("large file mode"), "{}", app.status);
    assert_workspace_status_is_display_safe(&app.status);
    assert_status_error_detail_is_bounded(&app.status);
}

#[test]
fn workspace_action_edit_batches_equivalent_open_buffer_paths_once() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let edit_path = root.join("src").join("..").join("src").join("main.rs");
    let mut app = app_for_test(root);
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path.clone()),
        "abcd\n".to_owned(),
    ));

    let outcome = app.apply_lsp_workspace_edits(
        vec![
            LspTextEdit {
                path,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "x".to_owned(),
            },
            LspTextEdit {
                path: edit_path,
                start_line: 1,
                start_column: 5,
                end_line: 1,
                end_column: 5,
                new_text: "y".to_owned(),
            },
        ],
        "Apply mixed path edit",
    );

    assert_eq!(outcome.open_changed, 1);
    assert_eq!(outcome.disk_queued, 0);
    assert_eq!(app.buffer(7).expect("buffer").text(), "xabcdy\n");
}

#[test]
fn workspace_action_edit_skips_dirty_buffer_with_pending_clean_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    let version = buffer.version();
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.in_flight_reloads.insert(
        7,
        PendingFileReload {
            request_id: 1,
            path: path.clone(),
            version,
            force_dirty: false,
        },
    );

    let outcome = app.apply_lsp_workspace_edits(vec![edit(&path, "changed\n")], "LSP edit");

    assert_eq!(outcome.open_changed, 0);
    assert_eq!(outcome.open_skipped, 1);
    assert_eq!(outcome.disk_queued, 0);
    assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
    assert_eq!(
        app.status,
        "LSP edit: changed 0 open buffers, skipped 1 unsafe open buffers"
    );
}

#[test]
fn workspace_action_edit_skips_dirty_buffer_with_queued_clean_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.queued_file_reloads.insert(
        7,
        QueuedFileReload {
            path: path.clone(),
            force_dirty: false,
        },
    );

    let outcome = app.apply_lsp_workspace_edits(vec![edit(&path, "changed\n")], "LSP edit");

    assert_eq!(outcome.open_changed, 0);
    assert_eq!(outcome.open_skipped, 1);
    assert_eq!(outcome.disk_queued, 0);
    assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
}

#[test]
fn workspace_action_edit_allows_force_dirty_pending_reload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    let version = buffer.version();
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.in_flight_reloads.insert(
        7,
        PendingFileReload {
            request_id: 1,
            path: path.clone(),
            version,
            force_dirty: true,
        },
    );

    let outcome = app.apply_lsp_workspace_edits(vec![edit(&path, "changed\n")], "LSP edit");

    assert_eq!(outcome.open_changed, 1);
    assert_eq!(outcome.open_skipped, 0);
    assert_eq!(outcome.disk_queued, 0);
    assert_eq!(
        app.buffer(7).expect("buffer").text(),
        "changed\nfn main() {}\n"
    );
}

#[test]
fn workspace_action_edit_skips_large_file_mode_open_buffer() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/large.rs");
    let large_text = std::iter::repeat_n("x", LARGE_FILE_MODE_MAX_LINES + 1)
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = app_for_test(root);
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path.clone()),
        large_text.clone(),
    ));

    let outcome = app.apply_lsp_workspace_edits(vec![edit(&path, "changed\n")], "Large edit");

    assert_eq!(outcome.open_changed, 0);
    assert_eq!(outcome.open_skipped, 1);
    assert_eq!(outcome.disk_queued, 0);
    assert_eq!(app.buffer(7).expect("buffer").text(), large_text);
    assert_eq!(
        app.status,
        "Large edit: changed 0 open buffers, skipped 1 unsafe open buffers"
    );
}
