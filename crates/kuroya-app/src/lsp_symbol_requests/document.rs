use crate::{
    KuroyaApp, lsp_runtime::lsp_command_queue_failed_status, path_display::display_path_label_cow,
};
#[cfg(test)]
use std::path::Path;

impl KuroyaApp {
    pub(crate) fn request_lsp_document_symbols(&mut self) {
        let Some((id, path, version, _, _)) = self.active_lsp_position() else {
            self.document_symbols.clear();
            self.document_symbols_path = None;
            self.document_symbols_selected = 0;
            self.status = "No LSP symbol target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.document_symbols.clear();
            self.document_symbols_path = Some(path.clone());
            self.document_symbols_selected = 0;
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.document_symbols(id, path.clone(), version) {
            self.document_symbols.clear();
            self.document_symbols_path = Some(path.clone());
            self.document_symbols_selected = 0;
            self.status = lsp_command_queue_failed_status("textDocument/documentSymbol");
            return;
        }
        let path_label = display_path_label_cow(&path);
        let status = lsp_document_symbols_request_status_for_label(path_label.as_ref());
        self.record_lsp_client_trace("textDocument/documentSymbol", path_label);
        self.document_symbols.clear();
        self.document_symbols_path = Some(path);
        self.document_symbols_selected = 0;
        self.status = status;
    }
}

#[cfg(test)]
fn lsp_document_symbols_request_status(path: &Path) -> String {
    let path_label = display_path_label_cow(path);
    lsp_document_symbols_request_status_for_label(path_label.as_ref())
}

fn lsp_document_symbols_request_status_for_label(path_label: &str) -> String {
    format!("Requesting symbols for {path_label}")
}

#[cfg(test)]
mod tests {
    use super::lsp_document_symbols_request_status;
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        lsp_client::LspClientHandle,
        path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspDocumentSymbol, TextBuffer, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn document_symbols_request_status_sanitizes_and_bounds_path_label() {
        let path = Path::new("workspace/src")
            .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(32)));

        let status = lsp_document_symbols_request_status(&path);
        let label = display_path_label_cow(&path);

        assert_eq!(status, format!("Requesting symbols for {}", label.as_ref()));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        assert!(label.contains("..."));
    }

    #[test]
    fn document_symbols_request_clears_stale_rows_after_queueing_current_request() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.active = Some(7);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {}\n".to_owned(),
        ));
        app.lsp_clients
            .insert("rust".to_owned(), LspClientHandle::accepting_for_test());
        app.document_symbols_path = Some(root.join("src/lib.rs"));
        app.document_symbols_selected = 12;
        app.document_symbols
            .push(stale_symbol(root.join("src/lib.rs")));

        app.request_lsp_document_symbols();

        assert!(app.document_symbols.is_empty());
        assert_eq!(app.document_symbols_path.as_deref(), Some(path.as_path()));
        assert_eq!(app.document_symbols_selected, 0);
        assert_eq!(
            app.status,
            format!("Requesting symbols for {}", display_path_label_cow(&path))
        );
    }

    fn stale_symbol(path: PathBuf) -> LspDocumentSymbol {
        LspDocumentSymbol {
            name: "Stale".to_owned(),
            detail: None,
            kind: 12,
            path,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 6,
            depth: 0,
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
