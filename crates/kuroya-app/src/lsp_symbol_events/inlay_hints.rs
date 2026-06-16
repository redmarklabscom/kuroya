use crate::{
    KuroyaApp,
    lsp_text_positions::lsp_one_based_utf16_column_to_char_column,
    path_display::{display_error_label_cow, display_path_label_cow},
    workspace_state::{buffer_id_path_version_matches, lsp_event_path_is_current},
};
use kuroya_core::{BufferId, LspInlayHint, TextBuffer};
use std::{fmt::Write as _, path::PathBuf};

pub(super) fn handle_inlay_hints_result(
    app: &mut KuroyaApp,
    id: BufferId,
    path: PathBuf,
    version: u64,
    hints: Option<Vec<LspInlayHint>>,
    error: Option<String>,
) {
    if !lsp_event_path_is_current(&app.workspace.root, &path)
        || !buffer_id_path_version_matches(&app.buffers, id, &path, version)
    {
        return;
    }
    if !app.settings.inlay_hints {
        app.inlay_hints.remove(&path);
        return;
    }
    if let Some(error) = error {
        app.inlay_hints.remove(&path);
        app.status = inlay_hints_failed_status(&error);
    } else if let Some(hints) = hints {
        let Some(buffer) = app.buffer(id) else {
            return;
        };
        let hints = valid_inlay_hints_for_buffer(buffer, hints);
        let count = hints.len();
        if hints.is_empty() {
            app.inlay_hints.remove(&path);
        } else {
            app.inlay_hints.insert(path.clone(), hints);
        }
        app.status = inlay_hints_loaded_status(count, &path);
    } else {
        app.inlay_hints.remove(&path);
        app.status = inlay_hints_load_failed_status(&path);
    }
}

fn inlay_hints_failed_status(error: &str) -> String {
    let error = display_error_label_cow(error);
    let mut status = String::with_capacity("Inlay hints failed: ".len() + error.len());
    status.push_str("Inlay hints failed: ");
    status.push_str(error.as_ref());
    status
}

fn inlay_hints_loaded_status(count: usize, path: &std::path::Path) -> String {
    let path = display_path_label_cow(path);
    let path = path.as_ref();
    if count == 0 {
        let mut status = String::with_capacity("No inlay hints in ".len() + path.len());
        status.push_str("No inlay hints in ");
        status.push_str(path);
        return status;
    }

    let mut status = String::with_capacity(24 + path.len());
    let _ = write!(status, "{count} inlay hints in {path}");
    status
}

fn inlay_hints_load_failed_status(path: &std::path::Path) -> String {
    let path = display_path_label_cow(path);
    let path = path.as_ref();
    let mut status = String::with_capacity("Could not load inlay hints for ".len() + path.len());
    status.push_str("Could not load inlay hints for ");
    status.push_str(path);
    status
}

fn valid_inlay_hints_for_buffer(
    buffer: &TextBuffer,
    hints: Vec<LspInlayHint>,
) -> Vec<LspInlayHint> {
    let mut valid_hints = Vec::with_capacity(hints.len());
    for mut hint in hints {
        let Some(char_column) =
            lsp_one_based_utf16_column_to_char_column(buffer, hint.line, hint.column)
        else {
            continue;
        };
        hint.column = char_column + 1;
        valid_hints.push(hint);
    }
    if valid_hints.len() > 1 {
        valid_hints.sort_by(|left, right| {
            left.line
                .cmp(&right.line)
                .then(left.column.cmp(&right.column))
                .then_with(|| left.label.cmp(&right.label))
                .then(left.kind.cmp(&right.kind))
        });
    }
    valid_hints
}

#[cfg(test)]
mod tests {
    use super::{handle_inlay_hints_result, valid_inlay_hints_for_buffer};
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspInlayHint, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn valid_inlay_hints_filter_out_of_bounds_positions() {
        let buffer = TextBuffer::from_text(1, None, "alpha\nbeta".to_owned());
        let hints = valid_inlay_hints_for_buffer(
            &buffer,
            vec![
                hint(1, 1, "start"),
                hint(1, 6, "end"),
                hint(1, 7, "past-line"),
                hint(3, 1, "missing-line"),
            ],
        );

        assert_eq!(hints, vec![hint(1, 1, "start"), hint(1, 6, "end")]);
    }

    #[test]
    fn valid_inlay_hints_convert_utf16_columns_to_char_columns() {
        let buffer = TextBuffer::from_text(1, None, "😀x".to_owned());
        let hints = valid_inlay_hints_for_buffer(
            &buffer,
            vec![
                hint(1, 1, "start"),
                hint(1, 3, "after-emoji"),
                hint(1, 2, "inside-surrogate"),
            ],
        );

        assert_eq!(hints, vec![hint(1, 1, "start"), hint(1, 2, "after-emoji")]);
    }

    #[test]
    fn valid_inlay_hints_sort_after_validation() {
        let buffer = TextBuffer::from_text(1, None, "alpha\nbeta\ngamma".to_owned());
        let hints = valid_inlay_hints_for_buffer(
            &buffer,
            vec![
                hint(2, 3, "middle-z"),
                hint(1, 5, "line-b"),
                hint(1, 5, "line-a"),
                hint(3, 1, "last"),
                hint(1, 1, "first"),
            ],
        );

        assert_eq!(
            hints,
            vec![
                hint(1, 1, "first"),
                hint(1, 5, "line-a"),
                hint(1, 5, "line-b"),
                hint(2, 3, "middle-z"),
                hint(3, 1, "last"),
            ]
        );
    }

    #[test]
    fn inlay_hint_error_status_sanitizes_and_bounds_lsp_error_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let (mut app, version) = app_with_lsp_buffer(root, path.clone());
        app.inlay_hints
            .insert(path.clone(), vec![hint(1, 1, "old")]);
        let raw_error = unsafe_status_text("first line\nsecond line");

        handle_inlay_hints_result(&mut app, 7, path.clone(), version, None, Some(raw_error));

        assert!(!app.inlay_hints.contains_key(&path));
        assert_safe_status_text(&app.status);
        assert_safe_status_error(&app.status, "Inlay hints failed: ");
    }

    #[test]
    fn inlay_hint_path_statuses_sanitize_and_bound_file_labels() {
        let root = PathBuf::from("workspace");
        let path = root.join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let (mut app, version) = app_with_lsp_buffer(root, path.clone());

        handle_inlay_hints_result(&mut app, 7, path.clone(), version, Some(Vec::new()), None);

        assert_safe_status_text(&app.status);
        assert_safe_status_path(&app.status, "No inlay hints in ");

        handle_inlay_hints_result(&mut app, 7, path, version, None, None);

        assert_safe_status_text(&app.status);
        assert_safe_status_path(&app.status, "Could not load inlay hints for ");
    }

    #[test]
    fn inlay_hint_count_status_sanitizes_path_and_keeps_raw_hint_labels() {
        let root = PathBuf::from("workspace");
        let path = root.join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let (mut app, version) = app_with_lsp_buffer(root, path.clone());
        let raw_label = unsafe_status_text("hint\nlabel");

        handle_inlay_hints_result(
            &mut app,
            7,
            path.clone(),
            version,
            Some(vec![hint(1, 1, &raw_label)]),
            None,
        );

        assert_safe_status_text(&app.status);
        assert_safe_status_path(&app.status, "1 inlay hints in ");
        let hints = app.inlay_hints.get(&path).expect("stored inlay hints");
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].label, raw_label);
        assert!(hints[0].label.contains('\n'));
        assert!(hints[0].label.contains('\u{202e}'));
    }

    fn hint(line: usize, column: usize, label: &str) -> LspInlayHint {
        LspInlayHint {
            line,
            column,
            label: label.to_owned(),
            kind: None,
        }
    }

    fn app_with_lsp_buffer(root: PathBuf, path: PathBuf) -> (KuroyaApp, u64) {
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path), "fn main() {}\n".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        app.settings.inlay_hints = true;
        (app, version)
    }

    fn unsafe_status_text(prefix: &str) -> String {
        format!(
            "{prefix}\u{202e}{}tail\u{2029}",
            "very-long-lsp-text-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "status contains unsafe display characters: {status:?}"
        );
    }

    fn assert_safe_status_error(status: &str, prefix: &str) {
        let error = status
            .rsplit_once(prefix)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("unexpected status: {status}"));

        assert!(error.contains("..."), "{status}");
        assert!(error.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    fn assert_safe_status_path(status: &str, prefix: &str) {
        let path = status
            .rsplit_once(prefix)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("unexpected status: {status}"));

        assert!(path.contains("..."), "{status}");
        assert!(path.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
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
}
