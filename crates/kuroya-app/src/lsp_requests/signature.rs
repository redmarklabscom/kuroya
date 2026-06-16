use crate::{
    KuroyaApp,
    lsp_runtime::{lsp_command_queue_failed_status, lsp_status_display_message},
    runtime_ticks::SIGNATURE_HELP_DEBOUNCE,
};
use eframe::egui::Context;
use kuroya_core::BufferId;
use std::time::Instant;

use super::completion::lsp_request_location_label;

impl KuroyaApp {
    pub(crate) fn request_lsp_signature_help(&mut self) {
        let Some(id) = self.active else {
            self.status = "No LSP signature target".to_owned();
            return;
        };
        self.request_lsp_signature_help_for_buffer(id, true);
    }

    pub(crate) fn schedule_lsp_signature_help_for_buffer(&mut self, ctx: &Context, id: BufferId) {
        self.pending_signature_help_requests
            .insert(id, Instant::now());
        ctx.request_repaint_after(SIGNATURE_HELP_DEBOUNCE);
    }

    pub(crate) fn request_lsp_signature_help_for_buffer(
        &mut self,
        id: BufferId,
        report_failures: bool,
    ) -> bool {
        self.pending_signature_help_requests.remove(&id);
        if !self.settings.parameter_hints_enabled {
            self.signature_help = None;
            if report_failures {
                self.status = "Signature help is disabled in settings".to_owned();
            }
            return false;
        }

        let Some((id, path, version, line, character)) = self.lsp_position_for_buffer(id) else {
            self.signature_help = None;
            if report_failures {
                self.status = "No LSP signature target".to_owned();
            }
            return false;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.signature_help = None;
            if report_failures {
                self.status = "No LSP server configured for this buffer".to_owned();
            }
            return false;
        };

        if !client.signature_help(id, path.clone(), version, line, character) {
            self.signature_help = None;
            if report_failures {
                self.status = lsp_command_queue_failed_status("textDocument/signatureHelp");
            }
            return false;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.completion_open = false;
        self.references_open = false;
        self.code_actions_open = false;
        self.signature_help = None;
        self.lsp_hover = None;
        self.status =
            lsp_status_display_message(&format!("Requesting signature help at {location_label}"));
        self.record_lsp_client_trace("textDocument/signatureHelp", location_label);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext, lsp_client::LspClientHandle,
        lsp_runtime::LSP_STATUS_MESSAGE_MAX_CHARS, path_display::DISPLAY_PATH_LABEL_MAX_CHARS,
        terminal::TerminalPane, transient_state::LspSignatureHelpPopup,
    };
    use kuroya_core::{EditorSettings, LanguageId, LspSignatureHelp, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn automatic_signature_help_request_clears_stale_popup_without_target() {
        let root = std::env::temp_dir().join("kuroya-signature-no-target-test");
        let stale_path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.signature_help = Some(signature_popup(7, stale_path));
        app.status = "unchanged".to_owned();

        assert!(!app.request_lsp_signature_help_for_buffer(7, false));

        assert!(app.signature_help.is_none());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn automatic_signature_help_request_clears_stale_popup_when_disabled() {
        let root = std::env::temp_dir().join("kuroya-signature-disabled-test");
        let stale_path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.settings.parameter_hints_enabled = false;
        app.signature_help = Some(signature_popup(7, stale_path));
        app.status = "unchanged".to_owned();

        assert!(!app.request_lsp_signature_help_for_buffer(7, false));

        assert!(app.signature_help.is_none());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn signature_help_request_status_is_bounded_and_keeps_raw_path_target() {
        let root = std::env::temp_dir().join("kuroya-signature-raw-path-test");
        let path = root.join("src").join(format!(
            "signature\n{}\u{202e}.rs",
            "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "fn main() {\n    call(arg\n}\n".to_owned(),
            LanguageId::Rust,
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 12));
        app.active = Some(7);
        app.buffers.push(buffer);
        app.lsp_clients
            .insert("rust".to_owned(), LspClientHandle::accepting_for_test());

        assert!(app.request_lsp_signature_help_for_buffer(7, true));

        assert_display_safe(&app.status);
        assert!(app.status.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);
        let trace = app.lsp_trace.back().expect("signature trace");
        assert_eq!(trace.method, "textDocument/signatureHelp");
        assert_display_safe(&trace.detail);
        assert_ne!(trace.detail, path.display().to_string());
    }

    fn signature_popup(id: u64, path: PathBuf) -> LspSignatureHelpPopup {
        LspSignatureHelpPopup {
            id,
            path,
            line: 1,
            column: 1,
            help: LspSignatureHelp {
                signatures: Vec::new(),
                active_signature: 0,
                active_parameter: None,
            },
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
