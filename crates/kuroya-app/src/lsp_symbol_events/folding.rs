use crate::{
    KuroyaApp,
    lsp_folding_runtime::{FoldingRangeSource, valid_folding_ranges_for_buffer},
    path_display::{display_error_label_cow, display_path_label_cow},
    workspace_state::{buffer_id_path_version_matches, lsp_event_path_is_current},
};
use kuroya_core::{BufferId, LspFoldingRange};
use std::path::{Path, PathBuf};

pub(super) fn handle_folding_ranges_result(
    app: &mut KuroyaApp,
    id: BufferId,
    path: PathBuf,
    version: u64,
    ranges: Option<Vec<LspFoldingRange>>,
    error: Option<String>,
) {
    if !lsp_event_path_is_current(&app.workspace.root, &path)
        || !buffer_id_path_version_matches(&app.buffers, id, &path, version)
    {
        return;
    }
    if let Some(error) = error {
        if !app.load_fallback_folding_ranges_for(id, path.clone()) {
            clear_pending_fold_line(app, &path);
            app.status = folding_ranges_failed_status(&error);
        }
    } else if let Some(ranges) = ranges {
        let Some(buffer) = app.buffer(id) else {
            return;
        };
        let ranges = valid_folding_ranges_for_buffer(buffer, ranges);
        if ranges.is_empty() && app.load_fallback_folding_ranges_for(id, path.clone()) {
            return;
        }
        app.apply_folding_ranges_for_path(path, ranges, FoldingRangeSource::Lsp);
    } else if !app.load_fallback_folding_ranges_for(id, path.clone()) {
        clear_pending_fold_line(app, &path);
        app.status = folding_ranges_load_failed_status(&path);
    }
}

fn folding_ranges_failed_status(error: &str) -> String {
    let error = display_error_label_cow(error);
    format!("Folding ranges failed: {}", error.as_ref())
}

fn folding_ranges_load_failed_status(path: &Path) -> String {
    let path = display_path_label_cow(path);
    format!("Could not load folding ranges for {}", path.as_ref())
}

fn clear_pending_fold_line(app: &mut KuroyaApp, path: &Path) {
    if app
        .pending_fold_line
        .as_ref()
        .is_some_and(|(pending_path, _)| pending_path == path)
    {
        app.pending_fold_line = None;
    }
}

#[cfg(test)]
mod tests {
    use super::handle_folding_ranges_result;
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspFoldingRange, TextBuffer, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn stale_folding_ranges_result_does_not_replace_current_cache() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "alpha\nbeta".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        app.folding_ranges
            .insert(path.clone(), vec![folding_range(1, 2)]);
        app.status = "current status".to_owned();

        handle_folding_ranges_result(
            &mut app,
            7,
            path.clone(),
            version + 1,
            Some(vec![folding_range(1, 3)]),
            None,
        );

        assert_eq!(
            app.folding_ranges.get(&path).map(Vec::as_slice),
            Some(&[folding_range(1, 2)][..])
        );
        assert_eq!(app.status, "current status");
    }

    #[test]
    fn folding_ranges_error_status_sanitizes_and_bounds_lsp_error_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let version = push_buffer(&mut app, 7, path.clone(), "alpha\nbeta\n");
        app.lossy_decoded_buffers.insert(7);
        let error = unsafe_error_text();

        handle_folding_ranges_result(&mut app, 7, path, version, None, Some(error.clone()));

        assert!(error.contains('\n'));
        assert!(error.contains('\u{202e}'));
        assert_safe_status_text(&app.status);
        assert!(app.status.starts_with("Folding ranges failed: "));
        assert!(app.status.contains("..."));
        assert!(
            app.status.chars().count()
                <= "Folding ranges failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn folding_ranges_load_failed_status_sanitizes_and_bounds_path_display_text() {
        let root = PathBuf::from("workspace");
        let path = unsafe_path_under(&root);
        let mut app = app_for_test(root);
        let version = push_buffer(&mut app, 7, path.clone(), "alpha\nbeta\n");
        app.lossy_decoded_buffers.insert(7);

        handle_folding_ranges_result(&mut app, 7, path.clone(), version, None, None);

        assert_raw_path_keeps_unsafe_text(&path);
        assert_safe_status_text(&app.status);
        assert!(app.status.starts_with("Could not load folding ranges for "));
        assert!(app.status.contains("..."));
        assert!(
            app.status.chars().count()
                <= "Could not load folding ranges for ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn folding_ranges_result_filters_invalid_provider_ranges() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "alpha\nbeta\ngamma".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);

        handle_folding_ranges_result(
            &mut app,
            7,
            path.clone(),
            version,
            Some(vec![
                folding_range(2, 5),
                folding_range(1, 3),
                folding_range(0, 2),
                folding_range(1, 3),
            ]),
            None,
        );

        assert_eq!(
            app.folding_ranges.get(&path).map(Vec::as_slice),
            Some(&[folding_range(1, 3)][..])
        );
        assert_eq!(app.status, "1 folding ranges in main.rs");
    }

    #[test]
    fn folding_ranges_result_preserves_raw_lsp_ranges() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let version = push_buffer(&mut app, 7, path.clone(), "alpha\nbeta\ngamma\n");
        let raw_kind = unsafe_status_text("region\nlabel");
        let raw_range = LspFoldingRange {
            start_line: 1,
            start_column: Some(2),
            end_line: 3,
            end_column: Some(4),
            kind: Some(raw_kind.clone()),
        };

        handle_folding_ranges_result(
            &mut app,
            7,
            path.clone(),
            version,
            Some(vec![raw_range.clone()]),
            None,
        );

        assert_eq!(
            app.folding_ranges.get(&path).map(Vec::as_slice),
            Some(&[raw_range][..])
        );
        assert_eq!(
            app.folding_ranges[&path][0].kind.as_deref(),
            Some(raw_kind.as_str())
        );
    }

    fn push_buffer(app: &mut KuroyaApp, id: u64, path: PathBuf, text: &str) -> u64 {
        let buffer = TextBuffer::from_text(id, Some(path), text.to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        version
    }

    fn unsafe_path_under(root: &Path) -> PathBuf {
        root.join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ))
    }

    fn unsafe_error_text() -> String {
        unsafe_status_text("first\nsecond")
    }

    fn unsafe_status_text(prefix: &str) -> String {
        format!(
            "{prefix}\u{202e}{}tail",
            "very-long-text-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn assert_raw_path_keeps_unsafe_text(path: &Path) {
        let raw = path.to_string_lossy();
        assert!(raw.contains('\n'));
        assert!(raw.contains('\u{202e}'));
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "status contains unsafe display characters: {status:?}"
        );
    }

    fn is_unsafe_status_char(ch: char) -> bool {
        ch.is_control()
            || matches!(
                ch,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
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

    fn folding_range(start_line: usize, end_line: usize) -> LspFoldingRange {
        LspFoldingRange {
            start_line,
            start_column: None,
            end_line,
            end_column: None,
            kind: None,
        }
    }
}
