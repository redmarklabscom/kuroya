use crate::{
    KuroyaApp,
    lsp_workspace_symbol_ranking::{
        MAX_WORKSPACE_SYMBOL_QUERY_MEMORY, record_workspace_symbol_query_memory,
    },
    path_display::{display_path_label_cow, sanitized_display_label_cow},
};
use kuroya_core::{LspCallHierarchyCall, LspReference, LspTypeHierarchyItem, LspWorkspaceSymbol};
use std::{borrow::Cow, path::Path};

const LSP_NAVIGATION_STATUS_NAME_MAX_CHARS: usize = 120;

impl KuroyaApp {
    pub(crate) fn open_reference(&mut self, reference: LspReference) {
        let LspReference {
            path, line, column, ..
        } = reference;
        self.references_open = false;
        self.references.clear();
        let status = lsp_reference_status(&path, line, column);
        if self.open_lsp_file_at(path, line, column) {
            self.status = status;
        }
    }

    pub(crate) fn open_workspace_symbol(&mut self, symbol: LspWorkspaceSymbol) {
        record_workspace_symbol_query_memory(
            &mut self.workspace_symbol_query_memory,
            &self.workspace.root,
            &self.workspace_symbol_submitted_query,
            &symbol,
            MAX_WORKSPACE_SYMBOL_QUERY_MEMORY,
        );
        let LspWorkspaceSymbol {
            name,
            path,
            line,
            column,
            ..
        } = symbol;
        self.workspace_symbols_open = false;
        self.workspace_symbols.clear();
        self.workspace_symbol_submitted_path = None;
        let status =
            lsp_named_navigation_status("Workspace symbol", &name, "symbol", &path, line, column);
        if self.open_lsp_file_at(path, line, column) {
            self.status = status;
        }
    }

    pub(crate) fn open_call_hierarchy_call(&mut self, call: LspCallHierarchyCall) {
        let item = call.item;
        let path = item.path;
        let name = item.name;
        let line = item.line;
        let column = item.column;
        self.call_hierarchy_open = false;
        let status = lsp_named_navigation_status("Call", &name, "call", &path, line, column);
        if self.open_lsp_file_at(path, line, column) {
            self.status = status;
        }
    }

    pub(crate) fn open_type_hierarchy_item(&mut self, item: LspTypeHierarchyItem) {
        let path = item.path;
        let name = item.name;
        let line = item.line;
        let column = item.column;
        self.type_hierarchy_open = false;
        let status = lsp_named_navigation_status("Type", &name, "type", &path, line, column);
        if self.open_lsp_file_at(path, line, column) {
            self.status = status;
        }
    }
}

fn lsp_reference_status(path: &Path, line: usize, column: usize) -> String {
    let path = display_path_label_cow(path);
    format!("Reference: {}:{line}:{column}", path.as_ref())
}

fn lsp_named_navigation_status(
    kind: &str,
    name: &str,
    fallback: &str,
    path: &Path,
    line: usize,
    column: usize,
) -> String {
    let name = lsp_navigation_status_name(name, fallback);
    let path = display_path_label_cow(path);
    format!(
        "{kind} `{}`: {}:{line}:{column}",
        name.as_ref(),
        path.as_ref()
    )
}

fn lsp_navigation_status_name<'a>(name: &'a str, fallback: &str) -> Cow<'a, str> {
    sanitized_display_label_cow(name, LSP_NAVIGATION_STATUS_NAME_MAX_CHARS, fallback)
}

#[cfg(test)]
mod tests {
    use super::{
        LSP_NAVIGATION_STATUS_NAME_MAX_CHARS, lsp_named_navigation_status,
        lsp_navigation_status_name, lsp_reference_status,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{
            DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow, sanitized_display_label,
        },
        terminal::TerminalPane,
    };
    use kuroya_core::{
        EditorSettings, LspCallHierarchyCall, LspCallHierarchyItem, LspReference,
        LspTypeHierarchyItem, LspWorkspaceSymbol, Workspace,
    };
    use serde_json::json;
    use std::{
        borrow::Cow,
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn lsp_navigation_statuses_sanitize_and_bound_paths_and_names() {
        let path = Path::new("workspace/src").join(format!(
            "bad\n{}\u{202e}main.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let name = format!(
            "Symbol\n{}\u{202e}Name",
            "very-long-name-".repeat(LSP_NAVIGATION_STATUS_NAME_MAX_CHARS)
        );

        let reference = lsp_reference_status(&path, 12, 4);
        let named = lsp_named_navigation_status("Workspace symbol", &name, "symbol", &path, 12, 4);

        assert_safe_status_text(&reference);
        assert_safe_status_text(&named);
        assert!(reference.contains("..."), "{reference}");
        assert!(named.contains("..."), "{named}");
        assert!(named.contains("Symbol"), "{named}");
        assert!(
            reference.chars().count()
                <= "Reference: :12:4".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );
        assert!(
            named.chars().count()
                <= "Workspace symbol ``: :12:4".chars().count()
                    + LSP_NAVIGATION_STATUS_NAME_MAX_CHARS
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn lsp_navigation_status_name_falls_back_after_sanitizing_blank_names() {
        let path = Path::new("workspace/src/main.rs");
        let status = lsp_named_navigation_status("Call", "\n\u{202e}\t", "call", path, 1, 1);

        assert_eq!(
            status,
            format!("Call `call`: {}:1:1", display_path_label_cow(path).as_ref())
        );
    }

    #[test]
    fn lsp_navigation_status_name_borrows_clean_ascii_and_unicode_names() {
        let ascii = lsp_navigation_status_name("OutsideSymbol", "symbol");
        let unicode = "Outside\u{03bb}Symbol";
        let unicode_label = lsp_navigation_status_name(unicode, "symbol");

        assert!(matches!(ascii, Cow::Borrowed("OutsideSymbol")));
        match unicode_label {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn lsp_navigation_status_name_owns_dirty_truncated_and_fallback_labels() {
        let dirty = lsp_navigation_status_name("Outside\n\u{202e}Symbol", "symbol");
        let blank = lsp_navigation_status_name("\n\u{202e}\t", "symbol");
        let long_name = "x".repeat(LSP_NAVIGATION_STATUS_NAME_MAX_CHARS + 1);
        let truncated = lsp_navigation_status_name(&long_name, "symbol");

        assert_eq!(dirty.as_ref(), "Outside Symbol");
        assert_eq!(blank.as_ref(), "symbol");
        assert!(truncated.contains("..."), "{truncated}");
        assert!(truncated.chars().count() <= LSP_NAVIGATION_STATUS_NAME_MAX_CHARS);
        assert!(matches!(dirty, Cow::Owned(_)));
        assert!(matches!(blank, Cow::Owned(_)));
        assert!(matches!(truncated, Cow::Owned(_)));
    }

    #[test]
    fn lsp_navigation_status_name_matches_sanitized_display_output() {
        let cases = [
            ("OutsideSymbol", "symbol"),
            ("Outside\u{03bb}Symbol", "symbol"),
            (" OutsideSymbol ", "symbol"),
            ("Outside\n\u{202e}Symbol", "symbol"),
            ("\n\u{202e}\t", "symbol"),
        ];

        for (name, fallback) in cases {
            assert_eq!(
                lsp_navigation_status_name(name, fallback).as_ref(),
                sanitized_display_label(name, LSP_NAVIGATION_STATUS_NAME_MAX_CHARS, fallback)
            );
        }

        let long_name = "x".repeat(LSP_NAVIGATION_STATUS_NAME_MAX_CHARS + 1);
        assert_eq!(
            lsp_navigation_status_name(&long_name, "symbol").as_ref(),
            sanitized_display_label(&long_name, LSP_NAVIGATION_STATUS_NAME_MAX_CHARS, "symbol")
        );
    }

    #[test]
    fn lsp_reference_action_keeps_rejected_navigation_status() {
        let root = PathBuf::from("workspace");
        let outside = PathBuf::from("outside/main.rs");
        let mut app = app_for_test(root);
        app.references_open = true;
        app.references.push(reference(outside.clone()));

        app.open_reference(reference(outside));

        assert!(!app.references_open);
        assert!(app.references.is_empty());
        assert_lsp_navigation_rejected_without_file_load(&app);
    }

    #[test]
    fn workspace_symbol_action_keeps_rejected_navigation_status() {
        let root = PathBuf::from("workspace");
        let outside = PathBuf::from("outside/main.rs");
        let mut app = app_for_test(root);
        app.workspace_symbols_open = true;
        app.workspace_symbols
            .push(workspace_symbol(outside.clone()));
        app.workspace_symbol_submitted_path = Some(PathBuf::from("workspace/src/main.rs"));

        app.open_workspace_symbol(workspace_symbol(outside));

        assert!(!app.workspace_symbols_open);
        assert!(app.workspace_symbols.is_empty());
        assert!(app.workspace_symbol_submitted_path.is_none());
        assert_lsp_navigation_rejected_without_file_load(&app);
    }

    #[test]
    fn call_hierarchy_action_keeps_rejected_navigation_status() {
        let root = PathBuf::from("workspace");
        let outside = PathBuf::from("outside/main.rs");
        let mut app = app_for_test(root);
        app.call_hierarchy_open = true;
        app.call_hierarchy_incoming.push(call(outside.clone()));

        app.open_call_hierarchy_call(call(outside));

        assert!(!app.call_hierarchy_open);
        assert_lsp_navigation_rejected_without_file_load(&app);
    }

    #[test]
    fn type_hierarchy_action_keeps_rejected_navigation_status() {
        let root = PathBuf::from("workspace");
        let outside = PathBuf::from("outside/main.rs");
        let mut app = app_for_test(root);
        app.type_hierarchy_open = true;
        app.type_hierarchy_supertypes
            .push(type_item(outside.clone()));

        app.open_type_hierarchy_item(type_item(outside));

        assert!(!app.type_hierarchy_open);
        assert_lsp_navigation_rejected_without_file_load(&app);
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

    fn reference(path: PathBuf) -> LspReference {
        LspReference {
            path,
            line: 12,
            column: 4,
            end_line: 12,
            end_column: 8,
        }
    }

    fn workspace_symbol(path: PathBuf) -> LspWorkspaceSymbol {
        LspWorkspaceSymbol {
            name: "OutsideSymbol".to_owned(),
            detail: None,
            kind: 12,
            path,
            line: 12,
            column: 4,
            end_line: 12,
            end_column: 8,
        }
    }

    fn call(path: PathBuf) -> LspCallHierarchyCall {
        LspCallHierarchyCall {
            item: call_item(path),
            ranges: Vec::new(),
        }
    }

    fn call_item(path: PathBuf) -> LspCallHierarchyItem {
        LspCallHierarchyItem {
            name: "outside_call".to_owned(),
            detail: None,
            kind: 12,
            path,
            line: 12,
            column: 4,
            end_line: 12,
            end_column: 8,
            raw: json!({"name": "outside_call"}),
        }
    }

    fn type_item(path: PathBuf) -> LspTypeHierarchyItem {
        LspTypeHierarchyItem {
            name: "OutsideType".to_owned(),
            detail: None,
            kind: 5,
            path,
            line: 12,
            column: 4,
            end_line: 12,
            end_column: 8,
            raw: json!({"name": "OutsideType"}),
        }
    }

    fn assert_lsp_navigation_rejected_without_file_load(app: &KuroyaApp) {
        assert!(app.pending_open_paths.is_empty());
        assert!(app.pending_file_jump.is_none());
        assert!(app.active_async_tasks.is_empty());
        assert!(app.async_task_trace.is_empty());
        assert_eq!(
            app.status,
            "Cannot open LSP location outside the workspace: main.rs"
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
}
