use crate::{
    KuroyaApp,
    lsp_symbol_events::position::lsp_position_within_buffer,
    path_display::{display_error_label_cow, display_path_label_cow},
    workspace_state::{
        active_buffer_path_matches, buffer_id_path_version_matches, lsp_event_path_is_current,
        paths_match_lexically,
    },
};
use kuroya_core::{BufferId, LspDocumentSymbol, TextBuffer};
use std::path::{Path, PathBuf};

pub(super) fn handle_document_symbols_result(
    app: &mut KuroyaApp,
    id: BufferId,
    path: PathBuf,
    version: u64,
    symbols: Option<Vec<LspDocumentSymbol>>,
    error: Option<String>,
) {
    if !lsp_event_path_is_current(&app.workspace.root, &path)
        || !buffer_id_path_version_matches(&app.buffers, id, &path, version)
    {
        return;
    }
    if app.symbols_panel && !active_buffer_path_matches(app.active_buffer(), &path) {
        return;
    }
    if let Some(error) = error {
        app.document_symbols.clear();
        app.document_symbols_path = Some(path);
        app.document_symbols_selected = 0;
        app.status = document_symbols_failed_status(&error);
    } else if let Some(symbols) = symbols {
        let Some(buffer) = app.buffer(id) else {
            return;
        };
        let symbols = valid_document_symbols_for_buffer(buffer, &path, symbols);
        let count = symbols.len();
        app.document_symbols = symbols;
        app.document_symbols_selected = 0;
        app.status = document_symbols_loaded_status(count, &path);
        app.document_symbols_path = Some(path);
    } else {
        app.document_symbols.clear();
        app.document_symbols_selected = 0;
        app.status = document_symbols_load_failed_status(&path);
        app.document_symbols_path = Some(path);
    }
}

fn document_symbols_failed_status(error: &str) -> String {
    let error = display_error_label_cow(error);
    format!("Document symbols failed: {}", error.as_ref())
}

fn document_symbols_loaded_status(count: usize, path: &Path) -> String {
    let path = display_path_label_cow(path);
    if count == 0 {
        format!("No symbols in {}", path.as_ref())
    } else {
        format!("{count} symbols in {}", path.as_ref())
    }
}

fn document_symbols_load_failed_status(path: &Path) -> String {
    let path = display_path_label_cow(path);
    format!("Could not load symbols for {}", path.as_ref())
}

fn valid_document_symbols_for_buffer(
    buffer: &TextBuffer,
    path: &Path,
    symbols: Vec<LspDocumentSymbol>,
) -> Vec<LspDocumentSymbol> {
    let mut valid_symbols = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        if document_symbol_range_is_valid_for_buffer(buffer, path, &symbol) {
            valid_symbols.push(symbol);
        }
    }
    valid_symbols
}

fn document_symbol_range_is_valid_for_buffer(
    buffer: &TextBuffer,
    path: &Path,
    symbol: &LspDocumentSymbol,
) -> bool {
    paths_match_lexically(&symbol.path, path)
        && document_symbol_range_order_is_valid(symbol)
        && lsp_position_within_buffer(buffer, symbol.line, symbol.column)
        && lsp_position_within_buffer(buffer, symbol.end_line, symbol.end_column)
}

fn document_symbol_range_order_is_valid(symbol: &LspDocumentSymbol) -> bool {
    symbol.end_line > symbol.line
        || (symbol.end_line == symbol.line && symbol.end_column >= symbol.column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn document_symbol_error_status_sanitizes_and_bounds_lsp_error_text() {
        let status = document_symbols_failed_status(&unsafe_error_text());

        assert_safe_status_text(&status);
        assert!(status.starts_with("Document symbols failed: "));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Document symbols failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn document_symbol_path_statuses_sanitize_and_bound_file_labels() {
        let path = unsafe_path();

        let loaded = document_symbols_loaded_status(3, &path);
        let empty = document_symbols_loaded_status(0, &path);
        let failed = document_symbols_load_failed_status(&path);

        for (prefix, status) in [
            ("3 symbols in ", loaded),
            ("No symbols in ", empty),
            ("Could not load symbols for ", failed),
        ] {
            assert_safe_status_text(&status);
            assert!(status.starts_with(prefix));
            assert!(status.contains("..."));
            assert!(
                status.chars().count() <= prefix.chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
            );
        }
    }

    #[test]
    fn document_symbol_result_preserves_raw_symbols_while_status_uses_path_label() {
        let root = PathBuf::from("workspace");
        let path = unsafe_path_under(&root);
        let mut app = app_for_test(root);
        let version = push_buffer(&mut app, 7, path.clone());
        let raw_name = unsafe_symbol_text("symbol");
        let raw_detail = unsafe_symbol_text("detail");
        let symbols = vec![LspDocumentSymbol {
            name: raw_name.clone(),
            detail: Some(raw_detail.clone()),
            kind: 12,
            path: path.clone(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 5,
            depth: 0,
        }];

        handle_document_symbols_result(&mut app, 7, path, version, Some(symbols), None);

        assert_safe_status_text(&app.status);
        assert!(app.status.starts_with("1 symbols in "));
        assert_eq!(app.document_symbols[0].name, raw_name);
        assert_eq!(
            app.document_symbols[0].detail.as_deref(),
            Some(raw_detail.as_str())
        );
    }

    #[test]
    fn document_symbol_result_filters_invalid_ranges_and_other_paths() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_buffer_with_text(&mut app, 7, path.clone(), "alpha\nbeta\n");
        let valid = symbol(path.clone(), "valid", 1, 1, 1, 5);

        handle_document_symbols_result(
            &mut app,
            7,
            path.clone(),
            version,
            Some(vec![
                symbol(path.clone(), "missing", 4, 1, 4, 2),
                symbol(path.clone(), "reversed", 2, 4, 2, 2),
                symbol(root.join("src/lib.rs"), "other", 1, 1, 1, 5),
                valid.clone(),
            ]),
            None,
        );

        assert_eq!(app.document_symbols, vec![valid]);
        assert_eq!(app.status, "1 symbols in main.rs");
    }

    fn push_buffer(app: &mut KuroyaApp, id: BufferId, path: PathBuf) -> u64 {
        push_buffer_with_text(app, id, path, "fn main() {}\n")
    }

    fn push_buffer_with_text(app: &mut KuroyaApp, id: BufferId, path: PathBuf, text: &str) -> u64 {
        let buffer = TextBuffer::from_text(id, Some(path), text.to_owned());
        let version = buffer.version();
        app.active = Some(id);
        app.buffers.push(buffer);
        version
    }

    fn symbol(
        path: PathBuf,
        name: &str,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
    ) -> LspDocumentSymbol {
        LspDocumentSymbol {
            name: name.to_owned(),
            detail: None,
            kind: 12,
            path,
            line,
            column,
            end_line,
            end_column,
            depth: 0,
        }
    }

    fn unsafe_path() -> PathBuf {
        unsafe_path_under(Path::new("workspace"))
    }

    fn unsafe_path_under(root: &Path) -> PathBuf {
        root.join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ))
    }

    fn unsafe_error_text() -> String {
        format!(
            "first\nsecond\u{202e}{}tail",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn unsafe_symbol_text(prefix: &str) -> String {
        format!(
            "{prefix}\nvalue\u{202e}{}tail",
            "very-long-symbol-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        )
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
}
