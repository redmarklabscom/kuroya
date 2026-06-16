use super::common::*;

#[test]
fn workspace_apply_edit_rejects_equivalent_create_over_open_buffer() {
    let root = temp_workspace("create-open-equivalent");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/main.rs");
    let create_path = root.join("src").join("..").join("src").join("main.rs");
    fs::write(&path, "disk\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        8,
        Some(path.clone()),
        "open\n".to_owned(),
    ));

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(39),
        label: Some("Create over open".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                path: create_path,
                overwrite: true,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(app.status.contains("resource create target is open"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "disk\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_equivalent_rename_affecting_open_buffer() {
    let root = temp_workspace("rename-open-equivalent");
    fs::create_dir_all(root.join("src")).unwrap();
    let old_path = root.join("src/main.rs");
    let rename_path = root.join("src").join("..").join("src").join("main.rs");
    let new_path = root.join("src/new.rs");
    fs::write(&old_path, "disk\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        8,
        Some(old_path.clone()),
        "open\n".to_owned(),
    ));

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(40),
        label: Some("Rename open".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                old_path: rename_path,
                new_path: new_path.clone(),
                overwrite: false,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(app.status.contains("resource rename affects open buffer"));
    assert_eq!(fs::read_to_string(&old_path).unwrap(), "disk\n");
    assert!(!new_path.exists());
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_equivalent_delete_affecting_open_buffer() {
    let root = temp_workspace("delete-open-equivalent");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/main.rs");
    let delete_path = root.join("src").join("..").join("src").join("main.rs");
    fs::write(&path, "disk\n").unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    app.buffers.push(TextBuffer::from_text(
        8,
        Some(path.clone()),
        "open\n".to_owned(),
    ));

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(41),
        label: Some("Delete open".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::DeleteFile {
                path: delete_path,
                recursive: false,
                ignore_if_not_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(app.status.contains("resource delete affects open buffer"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "disk\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_create_overwrite_binary_file_before_mutating_disk() {
    let root = temp_workspace("create-overwrite-binary");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/binary.rs");
    let original = b"old\0bytes".to_vec();
    fs::write(&path, &original).unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(37),
        label: Some("Overwrite binary".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                path: path.clone(),
                overwrite: true,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status
            .contains("resource create cannot overwrite unsafe file"),
        "{}",
        app.status
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_apply_edit_rejects_rename_overwrite_binary_file_before_mutating_disk() {
    let root = temp_workspace("rename-overwrite-binary");
    fs::create_dir_all(root.join("src")).unwrap();
    let old_path = root.join("src/old.rs");
    let target_path = root.join("src/target.rs");
    let original_target = b"old\0bytes".to_vec();
    fs::write(&old_path, "old\n").unwrap();
    fs::write(&target_path, &original_target).unwrap();
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(38),
        label: Some("Rename over binary".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::RenameFile {
                old_path: old_path.clone(),
                new_path: target_path.clone(),
                overwrite: true,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status
            .contains("resource rename cannot overwrite unsafe file"),
        "{}",
        app.status
    );
    assert_eq!(fs::read_to_string(&old_path).unwrap(), "old\n");
    assert_eq!(fs::read(&target_path).unwrap(), original_target);
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn workspace_apply_edit_rejects_case_equivalent_duplicate_create_before_mutating_disk() {
    let root = temp_workspace("create-case-duplicate");
    fs::create_dir_all(root.join("src")).unwrap();
    let first_path = root.join("src").join("Foo.rs");
    let second_path = root.join("src").join("foo.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(45),
        label: Some("Duplicate create".to_owned()),
        edits: Some(Vec::new()),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: first_path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: second_path,
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status.contains("resource create target already exists"),
        "{}",
        app.status
    );
    assert_workspace_status_is_display_safe(&app.status);
    assert_status_error_detail_is_bounded(&app.status);
    assert!(!first_path.exists());
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn workspace_apply_edit_applies_case_equivalent_create_and_text_edit() {
    let root = temp_workspace("create-case-edit");
    fs::create_dir_all(root.join("src")).unwrap();
    let create_path = root.join("src").join("Foo.rs");
    let edit_path = root.join("src").join("foo.rs");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(46),
        label: Some("Create case edit".to_owned()),
        edits: Some(vec![edit(&edit_path, "hello\n")]),
        document_changes: vec![
            LspWorkspaceDocumentChange::Resource(
                kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                    path: create_path.clone(),
                    overwrite: false,
                    ignore_if_exists: false,
                },
            ),
            LspWorkspaceDocumentChange::TextEdit {
                path: edit_path.clone(),
                version: None,
                edits: vec![edit(&edit_path, "hello\n")],
            },
        ],
        document_versions: BTreeMap::new(),
        error: None,
    });

    drain_until_lsp_workspace_event(&mut app);

    assert_eq!(fs::read_to_string(&create_path).unwrap(), "hello\n");
    drop(app);
    fs::remove_dir_all(root).unwrap();
}
