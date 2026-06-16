use super::common::*;

#[test]
fn workspace_apply_edit_rejection_status_sanitizes_server_error_and_preserves_raw_error() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );
    let raw_error = format!(
        "server\nerror \u{202e}{}",
        "e".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(18),
        label: None,
        edits: None,
        document_changes: Vec::new(),
        document_versions: BTreeMap::new(),
        error: Some(raw_error.clone()),
    });

    assert!(app.status.contains("server error"), "{}", app.status);
    assert!(app.status.contains("..."), "{}", app.status);
    assert_workspace_status_is_display_safe(&app.status);
    assert_status_error_detail_is_bounded(&app.status);
    assert!(raw_error.contains('\n'));
    assert!(raw_error.contains('\u{202e}'));
    assert!(raw_error.chars().count() > DISPLAY_ERROR_LABEL_MAX_CHARS);
}

#[test]
fn workspace_apply_edit_rejection_status_sanitizes_path_label_and_preserves_raw_path() {
    let root = PathBuf::from("workspace");
    let raw_file_name = format!(
        "bad\npath\u{202e}{}.rs",
        "p".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
    );
    let path = root.join("src").join(&raw_file_name);
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root,
        generation: 1,
        request_id: LspRequestId::Number(19),
        label: Some("Path rejection".to_owned()),
        edits: Some(Vec::new()),
        document_changes: Vec::new(),
        document_versions: BTreeMap::from([(path.clone(), 1)]),
        error: None,
    });

    assert!(
        app.status
            .contains("versioned workspace edit target is not open"),
        "{}",
        app.status
    );
    assert!(app.status.contains("..."), "{}", app.status);
    assert_workspace_status_is_display_safe(&app.status);
    assert_status_error_detail_is_bounded(&app.status);
    assert_eq!(path.file_name().unwrap().to_string_lossy(), raw_file_name);
    assert!(raw_file_name.contains('\n'));
    assert!(raw_file_name.contains('\u{202e}'));
    assert!(raw_file_name.chars().count() > DISPLAY_PATH_LABEL_MAX_CHARS);
}

#[test]
fn workspace_apply_edit_display_label_cow_borrows_clean_apply_edit_and_action_labels() {
    assert_eq!(
        workspace_apply_edit_display_label_cow(Some("Apply Workspace Edit")),
        Cow::Borrowed("Apply Workspace Edit")
    );
    assert_eq!(
        workspace_edit_action_display_label_cow("Rename Workspace Files"),
        Cow::Borrowed("Rename Workspace Files")
    );

    let apply_edit_unicode = "Apply workspace \u{03bb}";
    match workspace_apply_edit_display_label_cow(Some(apply_edit_unicode)) {
        Cow::Borrowed(label) => assert_eq!(label, apply_edit_unicode),
        Cow::Owned(label) => panic!("expected borrowed apply-edit label, got {label:?}"),
    }

    let action_unicode = "Rename workspace \u{30d5}\u{30a1}\u{30a4}\u{30eb}";
    match workspace_edit_action_display_label_cow(action_unicode) {
        Cow::Borrowed(label) => assert_eq!(label, action_unicode),
        Cow::Owned(label) => panic!("expected borrowed action label, got {label:?}"),
    }
}

#[test]
fn workspace_apply_edit_display_label_cow_owns_dirty_truncated_and_fallback_labels() {
    let dirty_apply_edit =
        workspace_apply_edit_display_label_cow(Some("Apply\nWorkspace Edit\u{202e}"));
    assert_owned_cow_eq(dirty_apply_edit, "Apply Workspace Edit");

    let dirty_action = workspace_edit_action_display_label_cow("Rename\nWorkspace Files\u{202e}");
    assert_owned_cow_eq(dirty_action, "Rename Workspace Files");

    let long = format!(
        "Apply workspace edit {}",
        "l".repeat(WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS * 2)
    );
    let truncated = workspace_apply_edit_display_label_cow(Some(&long));
    assert!(matches!(&truncated, Cow::Owned(_)));
    assert!(truncated.contains("..."), "{truncated}");
    assert!(truncated.chars().count() <= WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS);

    let action_long = format!(
        "Workspace action {}",
        "a".repeat(WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS * 2)
    );
    let truncated_action = workspace_edit_action_display_label_cow(&action_long);
    assert!(matches!(&truncated_action, Cow::Owned(_)));
    assert!(truncated_action.contains("..."), "{truncated_action}");
    assert!(truncated_action.chars().count() <= WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS);

    let fallback = workspace_apply_edit_display_label_cow(Some("\n\t\u{202e}"));
    assert_owned_cow_eq(fallback, "LSP workspace edit");

    let action_fallback = workspace_edit_action_display_label_cow("\n\t\u{202e}");
    assert_owned_cow_eq(action_fallback, "LSP workspace edit");
}

#[test]
fn workspace_apply_edit_display_label_wrappers_preserve_raw_inputs_and_visible_text() {
    let raw_label = format!(
        "Apply\nWorkspace Edit \u{202e}{}",
        "l".repeat(WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS * 2)
    );
    let label = workspace_apply_edit_display_label_cow(Some(&raw_label));

    assert!(matches!(&label, Cow::Owned(_)));
    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{202e}'));
    assert!(label.contains("..."), "{label}");
    assert_eq!(
        workspace_apply_edit_display_label(Some(&raw_label)),
        label.as_ref()
    );
    assert!(raw_label.contains('\n'));
    assert!(raw_label.contains('\u{202e}'));
    assert!(raw_label.chars().count() > WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS);

    assert_eq!(
        workspace_apply_edit_display_label(None),
        "LSP workspace edit"
    );
    assert_eq!(
        workspace_edit_action_display_label("Rename Workspace Files"),
        "Rename Workspace Files"
    );
}

#[test]
fn workspace_apply_edit_queued_status_sanitizes_label_and_preserves_raw_label() {
    let root = temp_workspace("label-sanitize");
    fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/new.rs");
    let raw_label = format!(
        "Apply\nWorkspace Edit \u{202e}{}",
        "l".repeat(WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS * 2)
    );
    let mut app = app_for_test(root.clone());
    app.lsp_clients.insert(
        "rust".to_owned(),
        LspClientHandle::disconnected_with_generation_for_test(1),
    );

    app.handle_lsp_edit_event(LspUiEvent::WorkspaceApplyEditRequest {
        language: "rust".to_owned(),
        root: root.clone(),
        generation: 1,
        request_id: LspRequestId::Number(20),
        label: Some(raw_label.clone()),
        edits: Some(Vec::new()),
        document_changes: vec![LspWorkspaceDocumentChange::Resource(
            kuroya_core::LspWorkspaceResourceOperation::CreateFile {
                path,
                overwrite: false,
                ignore_if_exists: false,
            },
        )],
        document_versions: BTreeMap::new(),
        error: None,
    });

    assert!(
        app.status.contains("Apply Workspace Edit"),
        "{}",
        app.status
    );
    assert!(app.status.contains("..."), "{}", app.status);
    assert!(
        app.status
            .ends_with(": queued 1 ordered workspace change(s)")
    );
    assert_workspace_status_is_display_safe(&app.status);
    assert!(
        app.status.chars().count()
            <= WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS
                + ": queued 1 ordered workspace change(s)".chars().count()
    );
    assert!(raw_label.contains('\n'));
    assert!(raw_label.contains('\u{202e}'));
    assert!(raw_label.chars().count() > WORKSPACE_APPLY_EDIT_LABEL_MAX_CHARS);
    drop(app);
    fs::remove_dir_all(root).unwrap();
}
