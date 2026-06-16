use crate::{
    KuroyaApp,
    lsp_hover_cache::{LspHoverCacheKey, MAX_LSP_HOVER_CACHE_ENTRIES, store_hover_cache},
    path_display::display_error_label_cow,
    transient_state::LspHoverPopup,
    workspace_state::buffer_id_path_version_matches,
    workspace_state::lsp_event_path_is_current,
};
use kuroya_core::{BufferId, LspDocumentHighlight};
use std::{path::PathBuf, time::Instant};

use super::active_lsp_navigation_response_matches;

impl KuroyaApp {
    pub(super) fn handle_lsp_hover_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        contents: Option<String>,
    ) {
        let hover_request_target_matches = self
            .lsp_hover_request
            .as_ref()
            .is_some_and(|target| target.matches(id, &path, version, line, column));
        let hover_request_matches = hover_request_target_matches
            && buffer_id_path_version_matches(&self.buffers, id, &path, version);
        let active_position_matches =
            active_lsp_navigation_response_matches(self, id, &path, version, line, column);
        if hover_request_target_matches && !hover_request_matches {
            self.lsp_hover_request = None;
        }
        if !lsp_event_path_is_current(&self.workspace.root, &path)
            || (!active_position_matches && !hover_request_matches)
        {
            return;
        }
        if hover_request_matches {
            self.lsp_hover_request = None;
        }
        if !self.settings.hover_enabled {
            self.lsp_hover = None;
            return;
        }
        let zero_based_column = column.saturating_sub(1);
        if let Some(contents) = contents {
            let now = Instant::now();
            store_hover_cache(
                &mut self.lsp_hover_cache,
                LspHoverCacheKey::new(path.clone(), version, line, zero_based_column),
                contents.clone(),
                MAX_LSP_HOVER_CACHE_ENTRIES,
            );
            self.lsp_hover = Some(LspHoverPopup {
                id,
                path,
                line: line + 1,
                column,
                contents,
                opened_at: now,
            });
            self.status = format!("Hover from LSP at {}:{column}", line + 1);
        } else {
            self.lsp_hover = None;
            self.status = format!("No hover at {}:{column}", line + 1);
        }
    }

    pub(super) fn handle_lsp_document_highlights_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        highlights: Option<Vec<LspDocumentHighlight>>,
        error: Option<String>,
    ) {
        if !active_lsp_navigation_response_matches(self, id, &path, version, line, column) {
            return;
        }
        if !self.settings.document_highlights_enabled {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            return;
        }
        if let Some(error) = error {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            self.status = document_highlights_failed_status(&error);
        } else if let Some(highlights) = highlights {
            let count = highlights.len();
            self.document_highlights_path = Some(path);
            self.document_highlights = highlights;
            self.status = if count == 0 {
                format!("No document highlights at {}:{}", line + 1, column)
            } else {
                format!("{count} document highlights at {}:{}", line + 1, column)
            };
        } else {
            self.document_highlights_path = None;
            self.document_highlights.clear();
            self.status = format!(
                "Could not load document highlights at {}:{}",
                line + 1,
                column
            );
        }
    }
}

fn document_highlights_failed_status(error: &str) -> String {
    format!(
        "Document highlights failed: {}",
        display_error_label_cow(error)
    )
}

#[cfg(test)]
mod tests {
    use super::document_highlights_failed_status;
    use crate::{
        KuroyaApp, app_startup_context::AppStartupContext,
        path_display::DISPLAY_ERROR_LABEL_MAX_CHARS, terminal::TerminalPane,
        transient_state::LspHoverRequestTarget,
    };
    use kuroya_core::{EditorSettings, TextBuffer, TextEdit, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn document_highlights_failure_status_sanitizes_and_bounds_provider_error() {
        let error = format!(
            "first line\nsecond line \u{202e}{}",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        );

        let status = document_highlights_failed_status(&error);

        assert!(
            !status.chars().any(is_unsafe_status_char),
            "unsafe status: {status:?}"
        );
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Document highlights failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
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

    #[test]
    fn hover_result_clears_stale_pointer_request_after_buffer_version_changes() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        app.lsp_hover_request = Some(LspHoverRequestTarget::from_request(
            7,
            path.clone(),
            version,
            0,
            3,
        ));
        app.status = "unchanged".to_owned();

        app.buffer_mut(7).expect("buffer").apply_edit(TextEdit {
            range: 0..0,
            inserted: "// ".to_owned(),
        });

        app.handle_lsp_hover_result(7, path, version, 0, 4, Some("hover docs".to_owned()));

        assert!(app.lsp_hover_request.is_none());
        assert!(app.lsp_hover.is_none());
        assert!(app.lsp_hover_cache.is_empty());
        assert_eq!(app.status, "unchanged");
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
