use crate::{
    KuroyaApp,
    lsp_runtime::{lsp_command_queue_failed_status, lsp_status_display_message},
    path_display::display_path_label_cow,
};
use eframe::egui::Context;
use kuroya_core::{BufferId, clamp_quick_suggestions_delay_ms};
use std::{
    fmt::Write as _,
    path::Path,
    time::{Duration, Instant},
};

impl KuroyaApp {
    pub(crate) fn request_lsp_completion(&mut self) {
        let Some(id) = self.active else {
            self.clear_completion_popup_state();
            self.status = "No LSP completion target".to_owned();
            return;
        };
        self.request_lsp_completion_for_buffer(id, true);
    }

    pub(crate) fn schedule_lsp_completion_for_buffer(&mut self, ctx: &Context, id: BufferId) {
        let delay_ms = clamp_quick_suggestions_delay_ms(self.settings.quick_suggestions_delay_ms);
        if delay_ms == 0 {
            self.pending_completion_requests.remove(&id);
            self.request_lsp_completion_for_buffer(id, false);
            return;
        }

        self.pending_completion_requests.insert(id, Instant::now());
        ctx.request_repaint_after(Duration::from_millis(delay_ms as u64));
    }

    pub(crate) fn request_lsp_completion_for_buffer(
        &mut self,
        id: BufferId,
        report_failures: bool,
    ) -> bool {
        self.pending_completion_requests.remove(&id);
        let Some((id, path, version, line, character)) = self.lsp_position_for_buffer(id) else {
            self.clear_completion_popup_state();
            if report_failures {
                self.status = "No LSP completion target".to_owned();
            }
            return false;
        };
        if !report_failures
            && self.completion_request_target_matches(id, &path, version, line, character)
        {
            return true;
        }
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.clear_completion_popup_state();
            if report_failures {
                self.status = "No LSP server configured for this buffer".to_owned();
            }
            return false;
        };

        if !client.completion(id, path.clone(), version, line, character) {
            self.clear_completion_popup_state();
            if report_failures {
                self.status = lsp_command_queue_failed_status("textDocument/completion");
            }
            return false;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.completion_open = true;
        self.completion_items.clear();
        self.completion_buffer_id = Some(id);
        self.completion_path = Some(path);
        self.completion_version = Some(version);
        self.completion_line = line + 1;
        self.completion_column = character + 1;
        self.completion_prefix.clear();
        self.completion_selected = 0;
        self.signature_help = None;
        let mut status =
            String::with_capacity("Requesting completions at ".len() + location_label.len());
        status.push_str("Requesting completions at ");
        status.push_str(&location_label);
        self.status = lsp_status_display_message(&status);
        self.record_lsp_client_trace("textDocument/completion", location_label);
        true
    }

    fn completion_request_target_matches(
        &self,
        id: BufferId,
        path: &Path,
        version: u64,
        line: usize,
        character: usize,
    ) -> bool {
        self.completion_open
            && self.completion_buffer_id == Some(id)
            && self.completion_path.as_deref() == Some(path)
            && self.completion_version == Some(version)
            && self.completion_line == line + 1
            && self.completion_column == character + 1
    }
}

pub(crate) fn lsp_request_location_label(path: &Path, line: usize, character: usize) -> String {
    let path = display_path_label_cow(path);
    let mut label = String::with_capacity(path.len() + 24);
    let _ = write!(label, "{}:{}:{}", path.as_ref(), line + 1, character + 1);
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext, lsp_client::LspClientHandle,
        lsp_runtime::LSP_STATUS_MESSAGE_MAX_CHARS, path_display::DISPLAY_PATH_LABEL_MAX_CHARS,
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LanguageId, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn lsp_request_location_label_is_display_safe_and_bounded() {
        let path = PathBuf::from("workspace/src")
            .join(format!("completion\n{}\u{2066}.rs", "segment-".repeat(24)));

        let label = lsp_request_location_label(&path, 6, 12);

        assert!(label.ends_with(":7:13"));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{2066}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS + ":7:13".chars().count());
    }

    #[test]
    fn completion_request_does_not_open_stale_popup_when_lsp_queue_is_full() {
        let root = std::env::temp_dir().join("kuroya-completion-queue-full-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text_with_language(
            7,
            Some(path),
            "fn main() {\n    pri\n}\n".to_owned(),
            LanguageId::Rust,
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 7));
        app.active = Some(7);
        app.buffers.push(buffer);
        app.completion_open = true;
        app.completion_prefix = "stale".to_owned();
        app.lsp_clients
            .insert("rust".to_owned(), LspClientHandle::full_queue_for_test());

        assert!(!app.request_lsp_completion_for_buffer(7, true));

        assert!(!app.completion_open);
        assert!(app.completion_items.is_empty());
        assert_eq!(
            app.status,
            lsp_command_queue_failed_status("textDocument/completion")
        );
    }

    #[test]
    fn automatic_completion_request_reuses_matching_target_without_queueing() {
        let root = std::env::temp_dir().join("kuroya-completion-same-target-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "fn main() {\n    pri\n}\n".to_owned(),
            LanguageId::Rust,
        );
        let version = buffer.version();
        buffer.set_single_cursor(buffer.line_column_to_char(1, 7));
        app.active = Some(7);
        app.buffers.push(buffer);
        app.completion_open = true;
        app.completion_buffer_id = Some(7);
        app.completion_path = Some(path);
        app.completion_version = Some(version);
        app.completion_line = 2;
        app.completion_column = 8;
        app.status = "unchanged".to_owned();
        app.lsp_clients
            .insert("rust".to_owned(), LspClientHandle::full_queue_for_test());

        assert!(app.request_lsp_completion_for_buffer(7, false));

        assert_eq!(app.status, "unchanged");
        assert!(app.completion_open);
    }

    #[test]
    fn automatic_completion_request_clears_stale_popup_without_target() {
        let root = std::env::temp_dir().join("kuroya-completion-no-target-test");
        let stale_path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.completion_open = true;
        app.completion_buffer_id = Some(7);
        app.completion_path = Some(stale_path);
        app.completion_version = Some(3);
        app.completion_line = 1;
        app.completion_column = 4;
        app.completion_prefix = "sta".to_owned();
        app.completion_selected = 2;
        app.status = "unchanged".to_owned();

        assert!(!app.request_lsp_completion_for_buffer(7, false));

        assert!(!app.completion_open);
        assert_eq!(app.completion_buffer_id, None);
        assert_eq!(app.completion_path, None);
        assert_eq!(app.completion_version, None);
        assert_eq!(app.completion_line, 0);
        assert_eq!(app.completion_column, 0);
        assert!(app.completion_prefix.is_empty());
        assert_eq!(app.completion_selected, 0);
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn automatic_completion_request_clears_stale_popup_without_server() {
        let root = std::env::temp_dir().join("kuroya-completion-no-server-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "fn main() {\n    pri\n}\n".to_owned(),
            LanguageId::Rust,
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 7));
        app.active = Some(7);
        app.buffers.push(buffer);
        app.lsp_unavailable.insert("rust".to_owned());
        app.completion_open = true;
        app.completion_buffer_id = Some(7);
        app.completion_path = Some(path);
        app.completion_version = Some(1);
        app.completion_line = 2;
        app.completion_column = 8;
        app.status = "unchanged".to_owned();

        assert!(!app.request_lsp_completion_for_buffer(7, false));

        assert!(!app.completion_open);
        assert_eq!(app.completion_buffer_id, None);
        assert_eq!(app.completion_path, None);
        assert_eq!(app.completion_version, None);
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn completion_request_status_is_bounded_and_keeps_raw_path_target() {
        let root = std::env::temp_dir().join("kuroya-completion-raw-path-test");
        let path = root.join("src").join(format!(
            "request\n{}\u{202e}.rs",
            "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "fn main() {\n    pri\n}\n".to_owned(),
            LanguageId::Rust,
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 7));
        app.active = Some(7);
        app.buffers.push(buffer);
        app.lsp_clients
            .insert("rust".to_owned(), LspClientHandle::accepting_for_test());

        assert!(app.request_lsp_completion_for_buffer(7, true));

        assert_eq!(app.completion_path.as_deref(), Some(path.as_path()));
        assert_display_safe(&app.status);
        assert!(app.status.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);
        let trace = app.lsp_trace.back().expect("completion trace");
        assert_eq!(trace.method, "textDocument/completion");
        assert_display_safe(&trace.detail);
        assert_ne!(trace.detail, path.display().to_string());
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

    fn assert_display_safe(value: &str) {
        assert!(!value.chars().any(char::is_control), "{value:?}");
        assert!(!value.chars().any(is_bidi_format_control), "{value:?}");
    }

    fn is_bidi_format_control(ch: char) -> bool {
        matches!(
            ch,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
    }
}
