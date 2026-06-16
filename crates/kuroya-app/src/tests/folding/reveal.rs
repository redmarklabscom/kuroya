use crate::{
    KuroyaApp, app_startup_context::AppStartupContext, folding::FoldedRange, terminal::TerminalPane,
};
use kuroya_core::{EditorSettings, TextBuffer, Workspace};
use std::{path::PathBuf, time::Instant};
use tokio::runtime::Runtime;

#[test]
fn file_jump_expands_folds_hiding_target_line() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut app = app_with_buffer(
        path.clone(),
        "one\ntwo\nthree\nfour\nfive\nsix\nseven".to_owned(),
    );
    app.folded_ranges.insert(
        path.clone(),
        vec![
            FoldedRange {
                start_line: 1,
                end_line: 7,
            },
            FoldedRange {
                start_line: 3,
                end_line: 4,
            },
            FoldedRange {
                start_line: 4,
                end_line: 5,
            },
            FoldedRange {
                start_line: 6,
                end_line: 7,
            },
        ],
    );

    app.apply_file_jump(1, 4, 1);

    assert_eq!(
        app.folded_ranges.get(&path).map(Vec::as_slice),
        Some(
            [
                FoldedRange {
                    start_line: 4,
                    end_line: 5,
                },
                FoldedRange {
                    start_line: 6,
                    end_line: 7,
                },
            ]
            .as_slice()
        )
    );
    assert_eq!(app.buffer(1).unwrap().cursor_position().line, 3);
    assert_eq!(app.pending_scroll_lines.get(&1), Some(&3));
}

#[test]
fn find_selection_expands_folds_hiding_match_line() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut app = app_with_buffer(path.clone(), "one\nneedle\nthree".to_owned());
    app.buffer_find_query = "needle".to_owned();
    app.folded_ranges.insert(
        path.clone(),
        vec![
            FoldedRange {
                start_line: 1,
                end_line: 3,
            },
            FoldedRange {
                start_line: 2,
                end_line: 3,
            },
        ],
    );

    assert!(app.select_find_match_with_result());

    assert_eq!(
        app.folded_ranges.get(&path).map(Vec::as_slice),
        Some(
            [FoldedRange {
                start_line: 2,
                end_line: 3,
            }]
            .as_slice()
        )
    );
    assert_eq!(app.buffer(1).unwrap().selections()[0].range(), 4..10);
    assert_eq!(app.pending_scroll_lines.get(&1), Some(&1));
}

fn app_with_buffer(path: PathBuf, text: String) -> KuroyaApp {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root);
    app.buffers.push(TextBuffer::from_text(1, Some(path), text));
    app.set_active_buffer(1);
    app
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
