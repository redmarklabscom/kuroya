use crate::{
    KuroyaApp,
    lsp_workspace_symbol_ranking::rank_workspace_symbols_by_navigation_context,
    path_display::{display_error_label_cow, sanitized_display_label_cow},
    workspace_state::lsp_event_path_is_current,
};
use kuroya_core::LspWorkspaceSymbol;
use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Path, PathBuf},
};

const WORKSPACE_SYMBOL_RESULT_QUERY_MAX_CHARS: usize = 160;
const WORKSPACE_SYMBOL_RESULT_MAX_ITEMS: usize = 200;

pub(super) fn handle_workspace_symbols_result(
    app: &mut KuroyaApp,
    path: PathBuf,
    query: String,
    symbols: Option<Vec<LspWorkspaceSymbol>>,
    error: Option<String>,
) {
    if !lsp_event_path_is_current(&app.workspace.root, &path) {
        return;
    }
    if !workspace_symbol_result_matches(
        app.workspace_symbols_open,
        &app.workspace_symbol_query,
        &app.workspace_symbol_submitted_query,
        app.workspace_symbol_submitted_path.as_deref(),
        &path,
        &query,
    ) {
        return;
    }
    if let Some(error) = error {
        app.load_index_workspace_symbols(query, &workspace_symbols_failed_reason(&error));
    } else if let Some(mut symbols) = symbols {
        symbols.retain(|symbol| lsp_event_path_is_current(&app.workspace.root, &symbol.path));
        retain_unique_workspace_symbols(&mut symbols);
        let total_count = symbols.len();
        if total_count == 0 {
            app.load_index_workspace_symbols(query, "No LSP workspace symbols");
        } else {
            symbols.truncate(WORKSPACE_SYMBOL_RESULT_MAX_ITEMS);
            let count = symbols.len();
            let open_file_paths = app
                .buffers
                .iter()
                .filter_map(|buffer| buffer.path().map(|path| path.as_path()))
                .collect::<Vec<_>>();
            rank_workspace_symbols_by_navigation_context(
                &mut symbols,
                &app.quick_open_recent_files,
                &open_file_paths,
                &app.workspace_symbol_query_memory,
                &query,
            );
            app.workspace_symbols = symbols;
            app.workspace_symbols_selected = 0;
            app.status = workspace_symbols_success_status(count, total_count, &query);
        }
    } else {
        app.load_index_workspace_symbols(query, "Could not load LSP workspace symbols");
    }
}

fn retain_unique_workspace_symbols(symbols: &mut Vec<LspWorkspaceSymbol>) {
    let mut seen = HashSet::with_capacity(symbols.len().min(WORKSPACE_SYMBOL_RESULT_MAX_ITEMS));
    symbols.retain(|symbol| {
        seen.insert((
            symbol.path.clone(),
            symbol.line,
            symbol.column,
            symbol.end_line,
            symbol.end_column,
            symbol.name.clone(),
        ))
    });
}

fn workspace_symbols_failed_reason(error: &str) -> String {
    format!(
        "Workspace symbols failed: {}",
        display_error_label_cow(error)
    )
}

fn workspace_symbols_success_status(count: usize, total_count: usize, query: &str) -> String {
    let query = workspace_symbol_result_query_label_cow(query);
    if count < total_count {
        format!(
            "Showing {count} of {total_count} workspace symbols for `{}`",
            query.as_ref()
        )
    } else {
        format!("{count} workspace symbols for `{}`", query.as_ref())
    }
}

#[cfg(test)]
fn workspace_symbol_result_query_label(query: &str) -> String {
    workspace_symbol_result_query_label_cow(query).into_owned()
}

fn workspace_symbol_result_query_label_cow(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        query,
        WORKSPACE_SYMBOL_RESULT_QUERY_MAX_CHARS,
        "workspace symbol",
    )
}

pub(crate) fn workspace_symbol_result_matches(
    open: bool,
    current_query: &str,
    submitted_query: &str,
    submitted_path: Option<&Path>,
    event_path: &Path,
    event_query: &str,
) -> bool {
    open && current_query.trim() == event_query
        && submitted_query == event_query
        && submitted_path.is_some_and(|path| path == event_path)
}

#[cfg(test)]
mod tests {
    use super::{
        WORKSPACE_SYMBOL_RESULT_MAX_ITEMS, WORKSPACE_SYMBOL_RESULT_QUERY_MAX_CHARS,
        handle_workspace_symbols_result, workspace_symbol_result_matches,
        workspace_symbol_result_query_label, workspace_symbol_result_query_label_cow,
        workspace_symbols_failed_reason, workspace_symbols_success_status,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspWorkspaceSymbol, Workspace};
    use std::{
        borrow::Cow,
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn workspace_symbol_results_match_open_query_and_submitted_path() {
        assert!(workspace_symbol_result_matches(
            true,
            " task ",
            "task",
            Some(Path::new("workspace/src/main.rs")),
            Path::new("workspace/src/main.rs"),
            "task",
        ));
    }

    #[test]
    fn workspace_symbol_results_ignore_stale_or_unsubmitted_results() {
        let path = Path::new("workspace/src/main.rs");
        assert!(!workspace_symbol_result_matches(
            false,
            "task",
            "task",
            Some(path),
            path,
            "task",
        ));
        assert!(!workspace_symbol_result_matches(
            true,
            "other",
            "task",
            Some(path),
            path,
            "task",
        ));
        assert!(!workspace_symbol_result_matches(
            true,
            "task",
            "other",
            Some(path),
            path,
            "task",
        ));
        assert!(!workspace_symbol_result_matches(
            true, "task", "task", None, path, "task",
        ));
        assert!(!workspace_symbol_result_matches(
            true,
            "task",
            "task",
            Some(Path::new("workspace/src/lib.rs")),
            path,
            "task",
        ));
    }

    #[test]
    fn workspace_symbol_success_status_sanitizes_and_bounds_query_display_text() {
        let query = format!(
            "Find\n{}\u{202e}Symbol",
            "very-long-query-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        );

        let status = workspace_symbols_success_status(7, 7, &query);

        assert_safe_status_text(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "7 workspace symbols for ``".chars().count()
                    + WORKSPACE_SYMBOL_RESULT_QUERY_MAX_CHARS
        );
        assert!(query.contains('\n'));
    }

    #[test]
    fn workspace_symbol_result_query_label_borrows_clean_ascii_and_unicode() {
        for query in ["TaskProvider", "symbol-\u{03bb}"] {
            match workspace_symbol_result_query_label_cow(query) {
                Cow::Borrowed(label) => assert_eq!(label, query),
                Cow::Owned(label) => panic!("expected borrowed query label, got {label:?}"),
            }
        }
    }

    #[test]
    fn workspace_symbol_result_query_label_owns_dirty_truncated_and_fallback_values() {
        let dirty = workspace_symbol_result_query_label_cow("Find\nSymbol\u{202e}");
        assert_eq!(dirty.as_ref(), "Find Symbol");
        assert!(matches!(&dirty, Cow::Owned(_)));

        let long = format!(
            "workspace-symbol-{}",
            "x".repeat(WORKSPACE_SYMBOL_RESULT_QUERY_MAX_CHARS)
        );
        let truncated = workspace_symbol_result_query_label_cow(&long);
        assert!(truncated.contains("..."), "{truncated}");
        assert!(truncated.chars().count() <= WORKSPACE_SYMBOL_RESULT_QUERY_MAX_CHARS);
        assert!(matches!(&truncated, Cow::Owned(_)));

        let fallback = workspace_symbol_result_query_label_cow("\n\t\u{202e}");
        assert_eq!(fallback.as_ref(), "workspace symbol");
        assert!(matches!(&fallback, Cow::Owned(_)));
    }

    #[test]
    fn workspace_symbol_result_query_label_string_wrapper_matches_cow_helper() {
        for query in ["TaskProvider", "Find\nSymbol\u{202e}", "\n\t\u{202e}"] {
            assert_eq!(
                workspace_symbol_result_query_label(query),
                workspace_symbol_result_query_label_cow(query).as_ref()
            );
        }
    }

    #[test]
    fn workspace_symbol_failed_reason_sanitizes_and_bounds_provider_error() {
        let error = format!(
            "first line\nsecond line \u{202e}{}",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        );

        let status = workspace_symbols_failed_reason(&error);

        assert_safe_status_text(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Workspace symbols failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn workspace_symbol_success_status_reports_truncated_result_count() {
        assert_eq!(
            workspace_symbols_success_status(200, 275, "task"),
            "Showing 200 of 275 workspace symbols for `task`"
        );
    }

    #[test]
    fn workspace_symbol_results_filter_destinations_outside_workspace() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let inside = root.join("src/lib.rs");
        let outside = PathBuf::from("outside/lib.rs");
        let mut app = app_for_test(root);
        app.workspace_symbols_open = true;
        app.workspace_symbol_query = "task".to_owned();
        app.workspace_symbol_submitted_query = "task".to_owned();
        app.workspace_symbol_submitted_path = Some(path.clone());

        handle_workspace_symbols_result(
            &mut app,
            path,
            "task".to_owned(),
            Some(vec![
                symbol(outside, "Outside"),
                symbol(inside.clone(), "Inside"),
            ]),
            None,
        );

        assert_eq!(app.workspace_symbols.len(), 1);
        assert_eq!(app.workspace_symbols[0].path, inside);
        assert_eq!(app.workspace_symbols[0].name, "Inside");
        assert_eq!(app.status, "1 workspace symbols for `task`");
    }

    #[test]
    fn workspace_symbol_results_dedupe_exact_symbol_locations() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let symbol_path = root.join("src/lib.rs");
        let mut app = app_for_test(root);
        app.workspace_symbols_open = true;
        app.workspace_symbol_query = "task".to_owned();
        app.workspace_symbol_submitted_query = "task".to_owned();
        app.workspace_symbol_submitted_path = Some(path.clone());

        handle_workspace_symbols_result(
            &mut app,
            path,
            "task".to_owned(),
            Some(vec![
                symbol(symbol_path.clone(), "Task"),
                symbol(symbol_path.clone(), "Task"),
                symbol(symbol_path.clone(), "OtherTask"),
            ]),
            None,
        );

        assert_eq!(app.workspace_symbols.len(), 2);
        assert_eq!(app.workspace_symbols[0].name, "Task");
        assert_eq!(app.workspace_symbols[1].name, "OtherTask");
        assert_eq!(app.status, "2 workspace symbols for `task`");
    }

    #[test]
    fn workspace_symbol_results_keep_raw_query_and_symbol_data_after_status_sanitization() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let symbol_path = root.join("src/raw.rs");
        let query = "Task\n\u{202e}Raw".to_owned();
        let raw_name = "Raw\n\u{202e}Symbol";
        let mut app = app_for_test(root);
        app.workspace_symbols_open = true;
        app.workspace_symbol_query = query.clone();
        app.workspace_symbol_submitted_query = query.clone();
        app.workspace_symbol_submitted_path = Some(path.clone());

        handle_workspace_symbols_result(
            &mut app,
            path,
            query.clone(),
            Some(vec![symbol(symbol_path.clone(), raw_name)]),
            None,
        );

        assert_eq!(app.workspace_symbol_query, query);
        assert_eq!(app.workspace_symbol_submitted_query, query);
        assert_eq!(app.workspace_symbols.len(), 1);
        assert_eq!(app.workspace_symbols[0].path, symbol_path);
        assert_eq!(app.workspace_symbols[0].name, raw_name);
        assert_safe_status_text(&app.status);
        assert!(!app.status.contains('\n'));
        assert!(!app.status.contains('\u{202e}'));
    }

    #[test]
    fn workspace_symbol_results_cap_lsp_provider_results() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.workspace_symbols_open = true;
        app.workspace_symbol_query = "task".to_owned();
        app.workspace_symbol_submitted_query = "task".to_owned();
        app.workspace_symbol_submitted_path = Some(path.clone());

        let symbols = (0..WORKSPACE_SYMBOL_RESULT_MAX_ITEMS + 17)
            .map(|index| {
                symbol(
                    root.join(format!("src/{index}.rs")),
                    &format!("Task{index}"),
                )
            })
            .collect::<Vec<_>>();

        handle_workspace_symbols_result(&mut app, path, "task".to_owned(), Some(symbols), None);

        assert_eq!(
            app.workspace_symbols.len(),
            WORKSPACE_SYMBOL_RESULT_MAX_ITEMS
        );
        assert_eq!(
            app.status,
            format!(
                "Showing {} of {} workspace symbols for `task`",
                WORKSPACE_SYMBOL_RESULT_MAX_ITEMS,
                WORKSPACE_SYMBOL_RESULT_MAX_ITEMS + 17
            )
        );
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "unsafe status: {status:?}"
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

    fn symbol(path: PathBuf, name: &str) -> LspWorkspaceSymbol {
        LspWorkspaceSymbol {
            name: name.to_owned(),
            detail: None,
            kind: 12,
            path,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 4,
        }
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
