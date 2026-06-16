use super::common::*;

#[test]
fn workspace_apply_edit_applies_ordered_create_and_text_edit() {
    let root = temp_workspace("create-edit");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/new.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(29),
        label: Some("Create file".to_owned()),
        edits: Some(vec![edit(&path, "hello\n")]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: path.clone(),
                version: None,
                edits: vec![edit(&path, "hello\n")],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_prefers_document_changes_over_top_level_changes() {
    let root = temp_workspace("document-changes-preferred");
    fs::create_dir_all(root.join("src")).unwrap();
    let open_path = root.join("src/main.rs");
    let created_path = root.join("src/new.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        9,
        Some(open_path.clone()),
        "fn main() {}\n".to_owned(),
    ));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "workspace/applyEdit",
        "params": {
            "label": "Mixed edit",
            "edit": {
                "changes": {
                    path_to_file_uri(&open_path): [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": "ignored\n"
                    }]
                },
                "documentChanges": [
                    {
                        "kind": "create",
                        "uri": path_to_file_uri(&created_path)
                    },
                    {
                        "textDocument": {
                            "uri": path_to_file_uri(&created_path),
                            "version": null
                        },
                        "edits": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "created\n"
                        }]
                    }
                ]
            }
        }
    });
    let request = parse_apply_workspace_edit_request(&value).unwrap();

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: request.id,
        label: request.label,
        edits: Some(request.edits),
        document_changes: request.document_changes,
        document_versions: request.document_versions,
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert_eq!(app.buffer(9).expect("buffer").text(), "fn main() {}\n");
    assert_eq!(fs::read_to_string(&created_path).unwrap(), "created\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_applies_ordered_rename_and_text_edit() {
    let root = temp_workspace("rename-edit");
    fs::create_dir_all(root.join("src")).unwrap();
    let old_path = root.join("src/old.rs");
    let new_path = root.join("src/new.rs");
    fs::write(&old_path, "old\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(31),
        label: Some("Rename file".to_owned()),
        edits: Some(vec![LspTextEdit {
            path: new_path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 4,
            new_text: "new".to_owned(),
        }]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                    old_path: old_path.clone(),
                    new_path: new_path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: new_path.clone(),
                version: None,
                edits: vec![LspTextEdit {
                    path: new_path.clone(),
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 4,
                    new_text: "new".to_owned(),
                }],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert!(!old_path.exists());
    assert_eq!(fs::read_to_string(&new_path).unwrap(), "new\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_allows_directory_rename_to_lexical_sibling() {
    let root = temp_workspace("rename-dir-sibling");
    let old_path = root.join("src");
    let new_path = root.join("src").join("..").join("renamed");
    let renamed_path = root.join("renamed");
    fs::create_dir_all(&old_path).unwrap();
    fs::write(old_path.join("main.rs"), "old\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(42),
        label: Some("Rename directory".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                old_path: old_path.clone(),
                new_path,
                overwrite: false,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert!(!old_path.exists());
    assert_eq!(
        fs::read_to_string(renamed_path.join("main.rs")).unwrap(),
        "old\n"
    );
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_directory_rename_to_lexical_child() {
    let root = temp_workspace("rename-dir-child");
    let old_path = root.join("src");
    let equivalent_old_path = root.join("src").join("..").join("src");
    let new_path = root.join("src").join("nested");
    fs::create_dir_all(&old_path).unwrap();
    fs::write(old_path.join("main.rs"), "old\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(43),
        label: Some("Rename directory".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                old_path: equivalent_old_path,
                new_path: new_path.clone(),
                overwrite: false,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status
            .contains("resource rename target is inside source"),
        "{}",
        app.status
    );
    assert_workspace_status_is_display_safe(&app.status);
    assert_status_error_detail_is_bounded(&app.status);
    assert_eq!(
        fs::read_to_string(old_path.join("main.rs")).unwrap(),
        "old\n"
    );
    assert!(!new_path.exists());
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_applies_directory_rename_then_child_text_edit() {
    let root = temp_workspace("rename-dir-edit-child");
    let old_dir = root.join("src").join("old");
    let new_dir = root.join("src").join("renamed");
    let old_child = old_dir.join("main.rs");
    let new_child = new_dir.join("main.rs");
    fs::create_dir_all(&old_dir).unwrap();
    fs::write(&old_child, "old\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(44),
        label: Some("Rename directory edit child".to_owned()),
        edits: Some(vec![LspTextEdit {
            path: new_child.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 4,
            new_text: "new".to_owned(),
        }]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                    old_path: old_dir.clone(),
                    new_path: new_dir.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: new_child.clone(),
                version: None,
                edits: vec![LspTextEdit {
                    path: new_child.clone(),
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 4,
                    new_text: "new".to_owned(),
                }],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert!(!old_dir.exists());
    assert_eq!(fs::read_to_string(&new_child).unwrap(), "new\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_create_then_invalid_text_edit_before_mutating_disk() {
    let root = temp_workspace("create-invalid-edit");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/new.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(34),
        label: Some("Create invalid".to_owned()),
        edits: Some(vec![LspTextEdit {
            path: path.clone(),
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 1,
            new_text: "late\n".to_owned(),
        }]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: path.clone(),
                version: None,
                edits: vec![LspTextEdit {
                    path: path.clone(),
                    start_line: 2,
                    start_column: 1,
                    end_line: 2,
                    end_column: 1,
                    new_text: "late\n".to_owned(),
                }],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status.contains("invalid LSP edit range"),
        "{}",
        app.status
    );
    assert!(!path.exists());
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_rename_then_invalid_text_edit_before_mutating_disk() {
    let root = temp_workspace("rename-invalid-edit");
    fs::create_dir_all(root.join("src")).unwrap();
    let old_path = root.join("src/old.rs");
    let new_path = root.join("src/new.rs");
    fs::write(&old_path, "old\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(35),
        label: Some("Rename invalid".to_owned()),
        edits: Some(vec![LspTextEdit {
            path: new_path.clone(),
            start_line: 99,
            start_column: 1,
            end_line: 99,
            end_column: 1,
            new_text: "late\n".to_owned(),
        }]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                    old_path: old_path.clone(),
                    new_path: new_path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: new_path.clone(),
                version: None,
                edits: vec![LspTextEdit {
                    path: new_path.clone(),
                    start_line: 99,
                    start_column: 1,
                    end_line: 99,
                    end_column: 1,
                    new_text: "late\n".to_owned(),
                }],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status.contains("invalid LSP edit range"),
        "{}",
        app.status
    );
    assert_eq!(fs::read_to_string(&old_path).unwrap(), "old\n");
    assert!(!new_path.exists());
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_applies_ordered_delete_recreate_and_text_edit() {
    let root = temp_workspace("delete-recreate-edit");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/recreated.rs");
    fs::write(&path, "old\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(36),
        label: Some("Recreate file".to_owned()),
        edits: Some(vec![edit(&path, "new\n")]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::DeleteFile {
                    path: path.clone(),
                    recursive: false,
                    ignore_if_not_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: path.clone(),
                version: None,
                edits: vec![edit(&path, "new\n")],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_ignores_missing_delete_when_requested() {
    let root = temp_workspace("delete-ignore");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("missing.rs");
    let created = root.join("created.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(32),
        label: Some("Delete missing".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::DeleteFile {
                    path,
                    recursive: false,
                    ignore_if_not_exists: true,
                },
            ),
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: created.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert!(created.exists());
    drop(app);
    fs::remove_dir_all(root).unwrap();
}
