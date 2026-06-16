use crate::{
    KuroyaApp,
    lsp_hover_cache::{LspHoverCacheKey, lookup_hover_cache_refresh},
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::display_path_label_cow,
    transient_state::{LspHoverPopup, LspHoverRequestTarget},
};
use std::{path::Path, time::Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoverRequestSource {
    Explicit,
    Pointer,
}

impl KuroyaApp {
    pub(crate) fn request_lsp_hover(&mut self) {
        self.pending_lsp_hover = None;
        self.lsp_hover_request = None;
        if !self.settings.hover_enabled {
            self.lsp_hover = None;
            self.status = "Hover is disabled in settings".to_owned();
            return;
        }
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.status = "No LSP hover target".to_owned();
            return;
        };
        self.request_lsp_hover_position(
            id,
            path,
            version,
            line,
            character,
            HoverRequestSource::Explicit,
        );
    }

    pub(crate) fn request_lsp_hover_for_buffer_char(
        &mut self,
        id: kuroya_core::BufferId,
        char_idx: usize,
    ) -> bool {
        if !self.settings.hover_enabled {
            self.lsp_hover = None;
            return false;
        }
        let Some((id, path, version, line, character)) =
            self.lsp_position_for_buffer_char(id, char_idx)
        else {
            return false;
        };
        self.request_lsp_hover_position(
            id,
            path,
            version,
            line,
            character,
            HoverRequestSource::Pointer,
        )
    }

    fn request_lsp_hover_position(
        &mut self,
        id: kuroya_core::BufferId,
        path: std::path::PathBuf,
        version: u64,
        line: usize,
        character: usize,
        source: HoverRequestSource,
    ) -> bool {
        if source == HoverRequestSource::Pointer
            && self
                .lsp_hover_request
                .as_ref()
                .is_some_and(|target| target.matches(id, &path, version, line, character + 1))
        {
            return true;
        }
        if source == HoverRequestSource::Pointer {
            self.lsp_hover_request = None;
        }
        let key = LspHoverCacheKey::new(path.clone(), version, line, character);
        if let Some(contents) = lookup_hover_cache_refresh(&mut self.lsp_hover_cache, &key) {
            let now = Instant::now();
            self.lsp_hover = Some(LspHoverPopup {
                id,
                path: path.clone(),
                line: line + 1,
                column: character + 1,
                contents,
                opened_at: now,
            });
            if source == HoverRequestSource::Explicit {
                self.status = format!(
                    "Hover from cache at {}",
                    lsp_request_location_label(&path, line, character)
                );
            }
            return true;
        }
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            if source == HoverRequestSource::Explicit {
                self.status = "No LSP server configured for this buffer".to_owned();
            }
            return false;
        };

        if !client.hover(id, path.clone(), version, line, character) {
            self.lsp_hover = None;
            self.lsp_hover_request = None;
            if source == HoverRequestSource::Explicit {
                self.status = lsp_command_queue_failed_status("textDocument/hover");
            }
            return false;
        }
        if source == HoverRequestSource::Pointer {
            self.lsp_hover_request = Some(LspHoverRequestTarget::from_request(
                id,
                path.clone(),
                version,
                line,
                character,
            ));
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.record_lsp_client_trace("textDocument/hover", location_label.clone());
        if source == HoverRequestSource::Explicit {
            self.status = format!("Requesting hover at {location_label}");
        }
        true
    }

    pub(crate) fn request_lsp_document_highlights(&mut self) {
        if !self.settings.document_highlights_enabled {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            self.status = "Document highlights are disabled in settings".to_owned();
            return;
        }
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            self.status = "No LSP highlight target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.document_highlights(id, path.clone(), version, line, character) {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            self.status = lsp_command_queue_failed_status("textDocument/documentHighlight");
            return;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.record_lsp_client_trace("textDocument/documentHighlight", location_label.clone());
        self.document_highlights_path = Some(path);
        self.document_highlights.clear();
        self.status = format!("Requesting document highlights at {location_label}");
    }

    pub(crate) fn request_lsp_definition(&mut self) {
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.status = "No LSP definition target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.definition(id, path.clone(), version, line, character) {
            self.status = lsp_command_queue_failed_status("textDocument/definition");
            return;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.record_lsp_client_trace("textDocument/definition", location_label.clone());
        self.status = format!("Requesting definition at {location_label}");
    }
}

pub(crate) fn lsp_request_location_label(path: &Path, line: usize, character: usize) -> String {
    format!(
        "{}:{}:{}",
        display_path_label_cow(path),
        line + 1,
        character + 1
    )
}

#[cfg(test)]
mod tests {
    use super::lsp_request_location_label;
    use crate::{
        KuroyaApp, app_startup_context::AppStartupContext, lsp_client::LspClientHandle,
        path_display::DISPLAY_PATH_LABEL_MAX_CHARS, terminal::TerminalPane,
        transient_state::LspHoverRequestTarget,
    };
    use kuroya_core::{EditorSettings, LanguageId, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn lsp_request_location_label_is_display_safe_and_bounded() {
        let path = PathBuf::from("workspace/src")
            .join(format!("hover\n{}\u{202e}.rs", "segment-".repeat(24)));

        let label = lsp_request_location_label(&path, 2, 4);

        assert!(label.ends_with(":3:5"));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS + ":3:5".chars().count());
    }

    #[test]
    fn pointer_hover_request_reuses_matching_target_without_queueing() {
        let root = std::env::temp_dir().join("kuroya-hover-same-target-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "abcdef".to_owned(),
            LanguageId::Rust,
        );
        let version = buffer.version();
        app.buffers.push(buffer);
        app.lsp_hover_request = Some(LspHoverRequestTarget::from_request(7, path, version, 0, 2));
        app.status = "unchanged".to_owned();
        app.lsp_clients
            .insert("rust".to_owned(), LspClientHandle::full_queue_for_test());

        assert!(app.request_lsp_hover_for_buffer_char(7, 2));

        assert_eq!(app.status, "unchanged");
        assert!(app.lsp_hover_request.is_some());
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
