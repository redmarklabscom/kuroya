use crate::{
    KuroyaApp,
    app_startup_context::AppStartupContext,
    lsp_code_actions::{
        MAX_CODE_ACTION_CONTEXT_DIAGNOSTICS, code_action_diagnostics_for_line,
        code_action_display_label, is_auto_import_code_action, sort_code_actions_for_display,
    },
    terminal::TerminalPane,
};
use kuroya_core::{
    Diagnostic, DiagnosticSet, DiagnosticSeverity, EditorSettings, LspCodeAction, LspTextEdit,
    TextBuffer, TextEdit, Workspace,
};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::runtime::Runtime;

fn edit_for_path(path: &Path) -> LspTextEdit {
    LspTextEdit {
        path: path.to_path_buf(),
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 1,
        new_text: "use std::collections::HashMap;\n".to_owned(),
    }
}

fn action(title: &str, kind: Option<&str>) -> LspCodeAction {
    action_for_path(Path::new("src/main.rs"), title, kind)
}

fn action_for_path(path: &Path, title: &str, kind: Option<&str>) -> LspCodeAction {
    LspCodeAction {
        title: title.to_owned(),
        kind: kind.map(str::to_owned),
        edits: vec![edit_for_path(path)],
        document_changes: Vec::new(),
        resolve_payload: None,
    }
}

fn diagnostic(path: &Path, source: &str, line: usize, message: &str) -> Diagnostic {
    Diagnostic {
        path: path.to_path_buf(),
        line,
        column: 5,
        char_range: Range { start: 4, end: 10 },
        severity: DiagnosticSeverity::Error,
        source: source.to_owned(),
        message: message.to_owned(),
        unused: false,
        deprecated: false,
    }
}

#[test]
fn loaded_code_action_applies_when_origin_still_current() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    buffer.set_single_cursor(0);
    let version = buffer.version();
    app.active = Some(7);
    app.buffers.push(buffer);
    seed_code_action_origin(&mut app, path.clone(), version);

    app.apply_code_action(action_for_path(&path, "Import HashMap", Some("quickfix")));

    assert_eq!(
        app.buffer(7).expect("buffer").text(),
        "use std::collections::HashMap;\nfn main() {}\n"
    );
    assert_eq!(
        app.status,
        "Applied code action `Import HashMap`: changed 1 open buffers"
    );
}

#[test]
fn loaded_code_action_rejects_stale_origin_after_buffer_changes() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_test(root);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    buffer.set_single_cursor(0);
    let origin_version = buffer.version();
    let end = buffer.len_chars();
    buffer.apply_edit(TextEdit {
        range: end..end,
        inserted: "// changed\n".to_owned(),
    });
    buffer.set_single_cursor(0);
    app.active = Some(7);
    app.buffers.push(buffer);
    seed_code_action_origin(&mut app, path.clone(), origin_version);

    app.apply_code_action(action_for_path(&path, "Import HashMap", Some("quickfix")));

    assert_eq!(
        app.buffer(7).expect("buffer").text(),
        "fn main() {}\n// changed\n"
    );
    assert_eq!(
        app.status,
        "Could not apply code action `Import HashMap`: target changed"
    );
}

#[test]
fn code_action_display_labels_identify_auto_imports() {
    let auto_import = action("Import `std::collections::HashMap`", Some("quickfix"));
    let source_import = action("Add all missing imports", Some("source.addMissingImports"));
    let refactor = action("Extract function", Some("refactor.extract"));

    assert!(is_auto_import_code_action(&auto_import));
    assert!(is_auto_import_code_action(&source_import));
    assert_eq!(
        code_action_display_label(&auto_import),
        "auto-import  Import `std::collections::HashMap`"
    );
    assert_eq!(
        code_action_display_label(&refactor),
        "refactor.extract  Extract function"
    );
}

#[test]
fn code_action_sorting_prioritizes_auto_import_quickfixes() {
    let mut actions = vec![
        action("Extract function", Some("refactor.extract")),
        action("Add semicolon", Some("quickfix")),
        action("Import `std::collections::HashMap`", Some("quickfix")),
        action("Fix all", Some("source.fixAll")),
    ];

    sort_code_actions_for_display(&mut actions);

    assert_eq!(
        actions
            .into_iter()
            .map(|action| action.title)
            .collect::<Vec<_>>(),
        vec![
            "Import `std::collections::HashMap`",
            "Add semicolon",
            "Fix all",
            "Extract function",
        ]
    );
}

#[test]
fn code_action_sorting_uses_cached_case_insensitive_title_keys() {
    let mut actions = vec![
        action("beta fix", Some("quickfix")),
        action("Alpha fix", Some("quickfix")),
        action("IMPORT `std::fmt::Debug`", Some("quickfix")),
    ];

    sort_code_actions_for_display(&mut actions);

    assert_eq!(
        actions
            .into_iter()
            .map(|action| action.title)
            .collect::<Vec<_>>(),
        vec!["IMPORT `std::fmt::Debug`", "Alpha fix", "beta fix"]
    );
}

#[test]
fn code_action_diagnostics_for_line_filters_static_and_stays_bounded() {
    let path = PathBuf::from("src/main.rs");
    let mut diagnostics = DiagnosticSet::default();
    let mut entries = vec![
        diagnostic(&path, "kuroya-static", 3, "TODO marker"),
        diagnostic(&path, "rust-analyzer", 2, "other line"),
    ];
    for index in 0..MAX_CODE_ACTION_CONTEXT_DIAGNOSTICS + 4 {
        entries.push(diagnostic(
            &path,
            "rust-analyzer",
            3,
            &format!("missing import {index}"),
        ));
    }
    diagnostics.replace(path.clone(), entries);

    let context = code_action_diagnostics_for_line(&diagnostics, &path, 3);

    assert_eq!(context.len(), MAX_CODE_ACTION_CONTEXT_DIAGNOSTICS);
    assert!(context.iter().all(|diagnostic| {
        diagnostic.source == "rust-analyzer" && diagnostic.message.starts_with("missing import")
    }));
}

fn seed_code_action_origin(app: &mut KuroyaApp, path: PathBuf, version: u64) {
    app.code_actions_open = true;
    app.code_actions_buffer_id = Some(7);
    app.code_actions_path = Some(path);
    app.code_actions_version = Some(version);
    app.code_actions_line = 1;
    app.code_actions_column = 1;
    app.code_actions_selected = 0;
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
