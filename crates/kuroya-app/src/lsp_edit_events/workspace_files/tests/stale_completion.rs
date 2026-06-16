use super::common::*;

#[test]
fn stale_workspace_edit_disk_completion_is_ignored_after_workspace_reset() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    let stale_generation = app.workspace_event_generation;
    app.status = "before".to_owned();
    app.reset_open_workspace_state();

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceEditFilesApplied {
        root,
        generation: stale_generation,
        changed: 2,
        failed: Vec::new(),
        apply_edit_response: None,
    });

    assert_eq!(app.status, "before");
    assert_eq!(app.workspace_index_in_flight_request_id, None);
    assert_eq!(app.git_scan_in_flight_request_id, None);
}

#[test]
fn workspace_edit_disk_completion_from_other_root_is_ignored() {
    let root = PathBuf::from("workspace");
    let other_root = PathBuf::from("other-workspace");
    let mut app = app_for_test(root);
    app.status = "before".to_owned();

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceEditFilesApplied {
        root: other_root,
        generation: app.workspace_event_generation,
        changed: 2,
        failed: Vec::new(),
        apply_edit_response: None,
    });

    assert_eq!(app.status, "before");
    assert_eq!(app.workspace_index_in_flight_request_id, None);
    assert_eq!(app.git_scan_in_flight_request_id, None);
}

#[test]
fn workspace_edit_disk_completion_without_changes_skips_refresh_work() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceEditFilesApplied {
        root,
        generation: app.workspace_event_generation,
        changed: 0,
        failed: Vec::new(),
        apply_edit_response: None,
    });

    assert_eq!(app.status, "Applied LSP edits to 0 files on disk");
    assert_eq!(app.workspace_index_in_flight_request_id, None);
    assert_eq!(app.git_scan_in_flight_request_id, None);
}

#[test]
fn workspace_apply_edit_rejects_stale_document_version_before_mutating_buffer() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    let stale_version = buffer.version().saturating_add(1);
    app.buffers.push(buffer);

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(17),
        label: Some("Apply stale edit".to_owned()),
        edits: Some(vec![edit(&path, "mod changed;\n")]),
        document_changes: Vec::new(),
        document_versions: BTreeMap::from([(path.clone(), stale_version as i32)]),
        error: None,
    });

    assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
}
