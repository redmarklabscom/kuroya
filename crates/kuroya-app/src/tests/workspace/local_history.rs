use crate::{
    KuroyaApp, app_startup_context::AppStartupContext, file_history::LOCAL_HISTORY_MAX_BYTES,
    terminal::TerminalPane,
};
use kuroya_core::{EditorSettings, LanguageId, Workspace};
use std::{
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::runtime::Runtime;

#[test]
fn local_history_loaded_opens_snapshot_as_read_only_virtual_revision_buffer() {
    let root = temp_root("local-history-valid-loaded");
    let path = root.join("src/main.rs");
    let snapshot_path = root.join(".kuroya/history/src/7.main.rs.bak");
    let mut app = app_for_test(root.clone());

    app.apply_local_history_loaded(
        root,
        app.workspace_event_generation,
        path,
        snapshot_path,
        7,
        "fn old() {}\n".to_owned(),
    );

    let id = app
        .active
        .expect("local history buffer should become active");
    let buffer = app
        .buffer(id)
        .expect("local history buffer should be opened");

    assert_eq!(app.buffers.len(), 1);
    assert_eq!(buffer.text(), "fn old() {}\n");
    assert!(buffer.is_read_only());
    assert_eq!(buffer.path(), None);
    assert_eq!(buffer.language(), LanguageId::Rust);
    assert_eq!(
        app.virtual_buffer_labels.get(&id).map(String::as_str),
        Some("main.rs (Local History)")
    );
    assert!(!app.diff_buffer_sources.contains_key(&id));
    assert_eq!(
        app.status,
        "Opened local history for main.rs from 7.main.rs.bak"
    );
}

#[test]
fn local_history_loaded_rejects_binary_snapshot_text_without_opening_revision_buffer() {
    let root = temp_root("local-history-binary-loaded");
    let path = root.join("src/main.rs");
    let snapshot_path = root.join(".kuroya/history/src/1.main.rs.bak");
    let mut app = app_for_test(root.clone());

    app.apply_local_history_loaded(
        root,
        app.workspace_event_generation,
        path,
        snapshot_path,
        1,
        "old\0snapshot".to_owned(),
    );

    assert!(app.virtual_buffer_labels.is_empty());
    assert_eq!(
        app.status,
        "Could not open local history for main.rs: snapshot contains binary data"
    );
}

#[test]
fn local_history_loaded_rejects_oversized_snapshot_text_without_opening_revision_buffer() {
    let root = temp_root("local-history-oversized-loaded");
    let path = root.join("src/main.rs");
    let snapshot_path = root.join(".kuroya/history/src/1.main.rs.bak");
    let mut app = app_for_test(root.clone());

    app.apply_local_history_loaded(
        root,
        app.workspace_event_generation,
        path,
        snapshot_path,
        1,
        "x".repeat(usize::try_from(LOCAL_HISTORY_MAX_BYTES).unwrap() + 1),
    );

    assert!(app.virtual_buffer_labels.is_empty());
    assert_eq!(
        app.status,
        "Could not open local history for main.rs: snapshot exceeds local history size limit"
    );
}

fn app_for_test(root: PathBuf) -> KuroyaApp {
    let (tx, rx) = crate::ui_event_channel::ui_event_channel();
    let settings = EditorSettings::default();
    KuroyaApp::from_startup_context(AppStartupContext {
        runtime: Runtime::new().expect("test runtime"),
        tx,
        rx,
        workspace: Workspace::new(root.clone()),
        settings: settings.clone(),
        settings_panel_draft: settings,
        settings_editor_font_path: String::new(),
        settings_ui_font_path: String::new(),
        theme_picker_selected: 0,
        saved_session: None,
        terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
        watcher: None,
        recent_projects: Vec::new(),
        trusted_workspaces: vec![root],
        now: Instant::now(),
        startup_timings: Vec::new(),
    })
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("kuroya-{name}-{}-{nanos}", std::process::id()))
}
