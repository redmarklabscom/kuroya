use crate::{
    KuroyaApp,
    lsp_runtime::lsp_command_queue_failed_status,
    lsp_workspace_symbol_ranking::rank_workspace_symbols_by_navigation_context,
    path_display::{display_path_label_cow, sanitized_display_label_cow},
};
use kuroya_core::{LspWorkspaceSymbol, ProjectSymbol, TextBuffer};
use std::borrow::Cow;

const INDEX_WORKSPACE_SYMBOL_LIMIT: usize = 200;
const WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS: usize = 160;
const WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS: usize = 160;

impl KuroyaApp {
    pub(crate) fn begin_workspace_symbols(&mut self) {
        self.workspace_symbol_query = self
            .active_buffer()
            .and_then(TextBuffer::word_at_cursor)
            .unwrap_or_default();
        self.workspace_symbol_submitted_query.clear();
        self.workspace_symbol_submitted_path = None;
        self.workspace_symbols.clear();
        self.workspace_symbols_selected = 0;
        self.workspace_symbols_open = true;
        self.completion_open = false;
        self.references_open = false;
        self.code_actions_open = false;
        self.signature_help = None;
        self.lsp_hover = None;
        self.status = "Workspace symbols".to_owned();
    }

    pub(crate) fn request_lsp_workspace_symbols(&mut self) {
        let query = self.workspace_symbol_query.trim().to_owned();
        if query.is_empty() {
            self.workspace_symbols.clear();
            self.workspace_symbol_submitted_query.clear();
            self.workspace_symbol_submitted_path = None;
            self.workspace_symbols_selected = 0;
            self.status = "Enter a workspace symbol query".to_owned();
            return;
        }
        let Some((id, path, _, _, _)) = self.active_lsp_position() else {
            self.load_index_workspace_symbols(query, "No LSP workspace symbol target");
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.load_index_workspace_symbols(query, "No LSP server configured for this buffer");
            return;
        };

        if !client.workspace_symbols(id, path.clone(), query.clone()) {
            self.load_index_workspace_symbols(
                query,
                &lsp_command_queue_failed_status("workspace/symbol"),
            );
            return;
        }
        let path_label = display_path_label_cow(&path);
        let query_label = workspace_symbol_status_query_label(&query);
        self.record_lsp_client_trace(
            "workspace/symbol",
            format!("{} `{}`", path_label.as_ref(), query_label.as_ref()),
        );
        self.workspace_symbol_submitted_query = query.clone();
        self.workspace_symbol_submitted_path = Some(path);
        self.workspace_symbols.clear();
        self.workspace_symbols_selected = 0;
        self.workspace_symbols_open = true;
        self.status = workspace_symbol_search_status(&query);
    }

    pub(crate) fn load_index_workspace_symbols(&mut self, query: String, reason: &str) {
        let mut symbols = self
            .index
            .workspace_symbols(&query, INDEX_WORKSPACE_SYMBOL_LIMIT)
            .into_iter()
            .map(project_symbol_to_lsp)
            .collect::<Vec<_>>();
        let open_file_paths = self
            .buffers
            .iter()
            .filter_map(|buffer| buffer.path().map(|path| path.as_path()))
            .collect::<Vec<_>>();
        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &self.quick_open_recent_files,
            &open_file_paths,
            &self.workspace_symbol_query_memory,
            &query,
        );
        let count = symbols.len();
        self.workspace_symbol_submitted_query = query.clone();
        self.workspace_symbol_submitted_path = None;
        self.workspace_symbols = symbols;
        self.workspace_symbols_selected = 0;
        self.workspace_symbols_open = true;
        self.status = indexed_workspace_symbol_status(count, &query, reason);
    }
}

fn project_symbol_to_lsp(symbol: ProjectSymbol) -> LspWorkspaceSymbol {
    LspWorkspaceSymbol {
        end_column: symbol.column + symbol.name.chars().count(),
        end_line: symbol.line,
        column: symbol.column,
        line: symbol.line,
        path: symbol.path,
        kind: symbol.kind.lsp_kind(),
        detail: Some(symbol.relative_path.display().to_string()),
        name: symbol.name,
    }
}

pub(crate) fn indexed_workspace_symbol_status(count: usize, query: &str, reason: &str) -> String {
    let reason = workspace_symbol_status_reason_label(reason);
    let query = workspace_symbol_status_query_label(query);
    match count {
        0 => format!("{reason}; no indexed workspace symbols for `{query}`"),
        1 => format!("{reason}; 1 indexed workspace symbol for `{query}`"),
        _ => format!("{reason}; {count} indexed workspace symbols for `{query}`"),
    }
}

fn workspace_symbol_search_status(query: &str) -> String {
    format!(
        "Searching workspace symbols for `{}`",
        workspace_symbol_status_query_label(query)
    )
}

fn workspace_symbol_status_query_label(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        query,
        WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS,
        "workspace symbol",
    )
}

fn workspace_symbol_status_reason_label(reason: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        reason,
        WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS,
        "workspace symbols unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS, WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS,
        indexed_workspace_symbol_status, workspace_symbol_search_status,
        workspace_symbol_status_query_label, workspace_symbol_status_reason_label,
    };
    use std::borrow::Cow;

    #[test]
    fn indexed_workspace_symbol_status_reports_reason_query_and_count() {
        assert_eq!(
            indexed_workspace_symbol_status(0, "task", "No LSP server"),
            "No LSP server; no indexed workspace symbols for `task`"
        );
        assert_eq!(
            indexed_workspace_symbol_status(1, "task", "No LSP server"),
            "No LSP server; 1 indexed workspace symbol for `task`"
        );
        assert_eq!(
            indexed_workspace_symbol_status(3, "task", "No LSP server"),
            "No LSP server; 3 indexed workspace symbols for `task`"
        );
    }

    #[test]
    fn workspace_symbol_request_statuses_sanitize_and_bound_query_and_reason() {
        let query = format!("task\n{}\u{202e}", "very-long-".repeat(32));
        let reason = format!("No LSP\n{}\u{202e}", "reason-".repeat(40));

        let search = workspace_symbol_search_status(&query);
        let indexed = indexed_workspace_symbol_status(0, &query, &reason);

        assert!(!search.contains('\n'));
        assert!(!search.contains('\u{202e}'));
        assert!(search.contains("..."));
        assert!(!indexed.contains('\n'));
        assert!(!indexed.contains('\u{202e}'));
        assert!(indexed.contains("..."));
        assert!(
            search.chars().count()
                <= "Searching workspace symbols for ``".chars().count()
                    + WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS
        );
        assert!(
            indexed.chars().count()
                <= "; no indexed workspace symbols for ``".chars().count()
                    + WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS
                    + WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS
        );
    }

    #[test]
    fn workspace_symbol_status_labels_borrow_clean_ascii_and_unicode() {
        for query in ["task", "symbol-\u{03bb}"] {
            match workspace_symbol_status_query_label(query) {
                Cow::Borrowed(label) => assert_eq!(label, query),
                Cow::Owned(label) => panic!("expected borrowed query label, got {label:?}"),
            }
        }

        for reason in ["No LSP server", "reason-\u{03bb}"] {
            match workspace_symbol_status_reason_label(reason) {
                Cow::Borrowed(label) => assert_eq!(label, reason),
                Cow::Owned(label) => panic!("expected borrowed reason label, got {label:?}"),
            }
        }
    }

    #[test]
    fn workspace_symbol_status_labels_own_dirty_truncated_and_fallback_values() {
        let long_query = format!(
            "symbol-{}",
            "x".repeat(WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS)
        );
        let long_reason = format!(
            "reason-{}",
            "x".repeat(WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS)
        );
        let cases = [
            (
                workspace_symbol_status_query_label("task\nname\u{202e}"),
                WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS,
                "workspace symbol",
            ),
            (
                workspace_symbol_status_query_label(&long_query),
                WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS,
                "workspace symbol",
            ),
            (
                workspace_symbol_status_query_label("\n\t\u{202e}"),
                WORKSPACE_SYMBOL_STATUS_QUERY_MAX_CHARS,
                "workspace symbol",
            ),
            (
                workspace_symbol_status_reason_label("No LSP\nserver\u{202e}"),
                WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS,
                "workspace symbols unavailable",
            ),
            (
                workspace_symbol_status_reason_label(&long_reason),
                WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS,
                "workspace symbols unavailable",
            ),
            (
                workspace_symbol_status_reason_label("\n\t\u{202e}"),
                WORKSPACE_SYMBOL_STATUS_REASON_MAX_CHARS,
                "workspace symbols unavailable",
            ),
        ];

        for (label, max_chars, fallback) in cases {
            assert!(matches!(&label, Cow::Owned(_)));
            assert!(!label.contains('\n'));
            assert!(!label.contains('\u{202e}'));
            assert!(label.chars().count() <= max_chars);
            assert!(!label.is_empty());
            if label.as_ref() == fallback {
                assert!(!fallback.is_empty());
            }
        }
    }
}
