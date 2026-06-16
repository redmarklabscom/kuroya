use crate::{KuroyaApp, app_startup_context::AppStartupContext, terminal::TerminalPane};
pub(super) use crate::{
    app_state::{PendingFileReload, QueuedFileReload},
    file_runtime::loaded_text_buffer,
    folding::FoldedRange,
    ui_events::UiEvent,
};
pub(super) use kuroya_core::TextBuffer;
use kuroya_core::{
    BufferId, Diagnostic, DiagnosticSeverity, EditorSettings, LspFoldingRange, Workspace,
};
use std::{path::Path, time::Instant};
pub(super) use std::{path::PathBuf, time::Duration};
use tokio::runtime::Runtime;
pub(super) fn reserve_reload_for_test(
    app: &mut KuroyaApp,
    id: BufferId,
    path: PathBuf,
    version: u64,
    force_dirty: bool,
) {
    insert_in_flight_reload_for_test(app, id, 1, path, version, force_dirty);
}

pub(super) fn insert_in_flight_reload_for_test(
    app: &mut KuroyaApp,
    id: BufferId,
    request_id: u64,
    path: PathBuf,
    version: u64,
    force_dirty: bool,
) {
    assert!(app.buffer(id).is_some(), "test reload buffer must exist");
    assert!(
        app.in_flight_reloads
            .insert(
                id,
                PendingFileReload {
                    request_id,
                    path,
                    version,
                    force_dirty,
                },
            )
            .is_none(),
        "test reload reservation must not overwrite an in-flight reload"
    );
}

pub(super) fn insert_canceled_reload_for_test(
    app: &mut KuroyaApp,
    id: BufferId,
    pending: PendingFileReload,
) {
    let canceled = (id, pending);
    assert!(
        app.canceled_file_reloads.insert(canceled.clone()),
        "test canceled reload must not duplicate an existing tombstone"
    );
    app.canceled_file_reload_order.push_back(canceled);
}

pub(super) fn assert_in_flight_reload_for_test(
    app: &KuroyaApp,
    id: BufferId,
    request_id: u64,
    path: &Path,
    version: u64,
    force_dirty: bool,
) {
    let pending = app
        .in_flight_reloads
        .get(&id)
        .expect("test in-flight reload should exist");
    assert_eq!(pending.request_id, request_id);
    assert_eq!(pending.path.as_path(), path);
    assert_eq!(pending.version, version);
    assert_eq!(pending.force_dirty, force_dirty);
}

pub(super) fn assert_queued_reload_for_test(
    app: &KuroyaApp,
    id: BufferId,
    path: &Path,
    force_dirty: bool,
) {
    let queued = app
        .queued_file_reloads
        .get(&id)
        .expect("test queued reload should exist");
    assert_eq!(queued.path.as_path(), path);
    assert_eq!(queued.force_dirty, force_dirty);
}

pub(super) fn assert_only_canceled_reload_present_for_test(
    app: &KuroyaApp,
    id: BufferId,
    request_id: u64,
    path: &Path,
    version: u64,
    force_dirty: bool,
) {
    let matches = |entry: &(BufferId, PendingFileReload)| {
        let (canceled_id, pending) = entry;
        *canceled_id == id
            && pending.request_id == request_id
            && pending.path.as_path() == path
            && pending.version == version
            && pending.force_dirty == force_dirty
    };
    assert_eq!(app.canceled_file_reloads.len(), 1);
    assert_eq!(app.canceled_file_reload_order.len(), 1);
    assert!(app.canceled_file_reloads.iter().any(matches));
    assert!(app.canceled_file_reload_order.iter().any(matches));
}

pub(super) fn app_for_test(root: PathBuf) -> KuroyaApp {
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

pub(super) fn folding_range(start_line: usize, end_line: usize) -> LspFoldingRange {
    LspFoldingRange {
        start_line,
        start_column: None,
        end_line,
        end_column: None,
        kind: None,
    }
}

pub(super) fn static_diagnostic(path: &std::path::Path) -> Diagnostic {
    Diagnostic {
        path: path.to_path_buf(),
        line: 1,
        column: 1,
        char_range: 0..1,
        severity: DiagnosticSeverity::Warning,
        source: "kuroya-static".to_owned(),
        message: "static warning".to_owned(),
        unused: false,
        deprecated: false,
    }
}
