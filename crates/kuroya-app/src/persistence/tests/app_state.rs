use super::*;

#[test]
fn recent_projects_are_deduplicated_and_bounded() {
    let projects = (0..14)
        .map(|idx| PathBuf::from(format!("workspace-{idx}")))
        .chain([
            PathBuf::from("workspace-3"),
            PathBuf::new(),
            PathBuf::from("workspace-0"),
        ])
        .collect::<Vec<_>>();

    let normalized = normalize_recent_projects(projects);

    assert_eq!(normalized.len(), 12);
    assert_eq!(normalized[0], PathBuf::from("workspace-0"));
    assert_eq!(normalized[3], PathBuf::from("workspace-3"));
    assert_eq!(normalized[11], PathBuf::from("workspace-11"));
    assert!(!normalized.contains(&PathBuf::from("workspace-12")));
    assert!(!normalized.contains(&PathBuf::new()));
}

#[test]
fn recent_projects_deduplicate_lexically_equivalent_paths() {
    let workspace = PathBuf::from("workspace");
    let equivalent = workspace.join("src").join("..");
    let sibling = PathBuf::from("workspace-other");

    let normalized = normalize_recent_projects([
        equivalent.clone(),
        workspace,
        sibling.clone(),
        sibling.join("."),
    ]);

    assert_eq!(normalized, vec![equivalent, sibling]);
}

#[test]
fn recent_projects_skip_empty_startup_placeholder() {
    let placeholder = app_state_dir().join("empty-workspace");
    let workspace = PathBuf::from("workspace");

    let normalized = normalize_recent_projects([placeholder, workspace.clone()]);

    assert_eq!(normalized, vec![workspace]);
}

#[test]
fn trusted_workspaces_are_deduplicated_bounded_and_defaulted() {
    let trusted = (0..130)
        .map(|idx| PathBuf::from(format!("workspace-{idx}")))
        .chain([
            PathBuf::from("workspace-3"),
            PathBuf::new(),
            PathBuf::from("workspace-0"),
        ])
        .collect::<Vec<_>>();

    let normalized = normalize_trusted_workspaces(trusted);

    assert_eq!(normalized.len(), 128);
    assert_eq!(normalized[0], PathBuf::from("workspace-0"));
    assert_eq!(normalized[3], PathBuf::from("workspace-3"));
    assert_eq!(normalized[127], PathBuf::from("workspace-127"));
    assert!(!normalized.contains(&PathBuf::from("workspace-128")));
    assert!(!normalized.contains(&PathBuf::new()));
}

#[test]
fn trusted_workspaces_deduplicate_lexically_equivalent_paths() {
    let workspace = PathBuf::from("workspace");
    let equivalent = workspace.join("src").join("..");
    let sibling = PathBuf::from("workspace-other");

    let normalized = normalize_trusted_workspaces([
        equivalent.clone(),
        workspace.clone(),
        sibling.clone(),
        sibling.join("."),
    ]);

    assert_eq!(normalized, vec![equivalent, sibling]);
}

#[test]
fn app_state_round_trips_recent_projects_atomically() {
    let workspace = temp_workspace("app-state");
    fs::create_dir_all(&workspace).unwrap();
    let path = workspace.join("state.json");
    let first = AppState {
        recent_projects: vec![
            PathBuf::from("workspace-a"),
            PathBuf::from("workspace-b"),
            PathBuf::from("workspace-a"),
        ],
        trusted_workspaces: vec![
            PathBuf::from("workspace-a"),
            PathBuf::from("workspace-a"),
            PathBuf::new(),
        ],
        vim_keybindings: Some(true),
        vim: Some(kuroya_core::EditorVimSettings {
            disabled_bindings: vec!["Q".to_owned(), "<Nope>".to_owned()],
            key_overrides: vec![
                kuroya_core::EditorVimKeyOverride {
                    before: "<Home>".to_owned(),
                    after: "0".to_owned(),
                    command: None,
                },
                kuroya_core::EditorVimKeyOverride {
                    before: "L".to_owned(),
                    after: "<Left>".to_owned(),
                    command: None,
                },
            ],
        }),
    };
    let first_sanitized_vim = Some(kuroya_core::EditorVimSettings {
        disabled_bindings: vec!["Q".to_owned()],
        key_overrides: vec![kuroya_core::EditorVimKeyOverride {
            before: "<Home>".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    });
    let second = AppState {
        recent_projects: vec![PathBuf::from("workspace-c")],
        trusted_workspaces: vec![PathBuf::from("workspace-c")],
        vim_keybindings: Some(false),
        vim: Some(kuroya_core::EditorVimSettings {
            disabled_bindings: vec!["<C-n>".to_owned()],
            key_overrides: Vec::new(),
        }),
    };

    save_app_state_to_path(&path, &first).unwrap();
    assert_eq!(
        load_app_state_from_path(&path).unwrap().recent_projects,
        vec![PathBuf::from("workspace-a"), PathBuf::from("workspace-b")]
    );
    assert_eq!(
        load_app_state_from_path(&path).unwrap().trusted_workspaces,
        vec![PathBuf::from("workspace-a")]
    );
    assert_eq!(
        load_app_state_from_path(&path).unwrap().vim_keybindings,
        Some(true)
    );
    assert_eq!(
        load_app_state_from_path(&path).unwrap().vim,
        first_sanitized_vim
    );

    save_app_state_to_path(&path, &second).unwrap();
    assert_eq!(load_app_state_from_path(&path).unwrap(), second);
    assert_no_app_state_temps(&workspace);

    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn app_state_loads_old_files_without_vim_keybindings() {
    let workspace = temp_workspace("app-state-old-file");
    fs::create_dir_all(&workspace).unwrap();
    let path = workspace.join("state.json");
    fs::write(
        &path,
        r#"{
  "recent_projects": ["workspace-a"],
  "trusted_workspaces": []
}"#,
    )
    .unwrap();

    let loaded = load_app_state_from_path(&path).unwrap();

    assert_eq!(loaded.recent_projects, vec![PathBuf::from("workspace-a")]);
    assert_eq!(loaded.trusted_workspaces, Vec::<PathBuf>::new());
    assert_eq!(loaded.vim_keybindings, None);
    assert_eq!(loaded.vim, None);

    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn app_state_load_defaults_when_state_file_is_missing() {
    let workspace = temp_workspace("app-state-missing");
    fs::create_dir_all(&workspace).unwrap();
    let path = workspace.join("state.json");

    assert_eq!(
        load_app_state_from_path(&path).unwrap(),
        AppState::default()
    );
    assert!(!path.exists());
    assert!(quarantined_app_state_files(&workspace).is_empty());

    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn app_state_load_quarantines_corrupt_state_file_and_defaults() {
    let workspace = temp_workspace("app-state-corrupt");
    fs::create_dir_all(&workspace).unwrap();
    let path = workspace.join("state.json");
    fs::write(&path, "{not json").unwrap();

    assert_eq!(
        load_app_state_from_path(&path).unwrap(),
        AppState::default()
    );
    assert!(!path.exists());
    let quarantined = quarantined_app_state_files(&workspace);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(fs::read_to_string(&quarantined[0]).unwrap(), "{not json");

    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn app_state_load_quarantines_oversized_state_file_and_defaults() {
    let workspace = temp_workspace("app-state-oversized");
    fs::create_dir_all(&workspace).unwrap();
    let path = workspace.join("state.json");
    fs::write(
        &path,
        vec![b'a'; usize::try_from(APP_STATE_MAX_BYTES + 1).unwrap()],
    )
    .unwrap();

    assert_eq!(
        load_app_state_from_path(&path).unwrap(),
        AppState::default()
    );
    assert!(!path.exists());
    let quarantined = quarantined_app_state_files(&workspace);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(
        fs::metadata(&quarantined[0]).unwrap().len(),
        APP_STATE_MAX_BYTES + 1
    );

    fs::remove_dir_all(workspace).unwrap();
}

fn quarantined_app_state_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("state.json.corrupt."))
        })
        .collect()
}

fn assert_no_app_state_temps(dir: &Path) {
    let temp_count = fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.contains(".tmp."))
        })
        .count();
    assert_eq!(temp_count, 0);
}
