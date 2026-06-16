use crate::{
    KuroyaApp, lsp_code_actions::code_action_diagnostics_for_line,
    lsp_runtime::lsp_command_queue_failed_status, lsp_text_positions::lsp_line_content_utf16_len,
    path_display::display_path_label_cow,
};
use std::path::Path;

impl KuroyaApp {
    pub(crate) fn clear_lsp_code_action_state(&mut self) {
        self.code_actions_open = false;
        self.code_actions.clear();
        self.code_actions_buffer_id = None;
        self.code_actions_path = None;
        self.code_actions_version = None;
        self.code_actions_line = 0;
        self.code_actions_column = 0;
        self.code_actions_selected = 0;
    }

    pub(crate) fn request_lsp_code_actions(&mut self) {
        if !self.settings.lightbulb.enabled() {
            self.clear_lsp_code_action_state();
            self.status = "Code actions are disabled by the lightbulb setting".to_owned();
            return;
        }

        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.clear_lsp_code_action_state();
            self.status = "No LSP code action target".to_owned();
            return;
        };
        let Some(buffer) = self.buffer(id) else {
            self.clear_lsp_code_action_state();
            self.status = "No LSP code action target".to_owned();
            return;
        };
        let end_character = lsp_line_content_utf16_len(buffer, line).unwrap_or(character);
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.clear_lsp_code_action_state();
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        let diagnostics = code_action_diagnostics_for_line(&self.diagnostics, &path, line + 1);
        let location = lsp_code_action_request_location(&path, line, character);
        if !client.code_actions(
            id,
            path.clone(),
            version,
            line,
            character,
            line,
            0,
            line,
            end_character,
            diagnostics,
        ) {
            self.clear_lsp_code_action_state();
            self.status = lsp_command_queue_failed_status("textDocument/codeAction");
            return;
        }
        self.code_actions_open = true;
        self.code_actions.clear();
        self.code_actions_buffer_id = Some(id);
        self.code_actions_path = Some(path);
        self.code_actions_version = Some(version);
        self.code_actions_line = line + 1;
        self.code_actions_column = character + 1;
        self.code_actions_selected = 0;
        self.completion_open = false;
        self.signature_help = None;
        self.lsp_hover = None;
        self.status = lsp_code_action_request_status_for_location(&location);
        self.record_lsp_client_trace("textDocument/codeAction", location);
    }
}

fn lsp_code_action_request_location(path: &Path, line: usize, character: usize) -> String {
    format!(
        "{}:{}:{}",
        display_path_label_cow(path),
        line.saturating_add(1),
        character.saturating_add(1)
    )
}

#[cfg(test)]
fn lsp_code_action_request_status(path: &Path, line: usize, character: usize) -> String {
    lsp_code_action_request_status_for_location(&lsp_code_action_request_location(
        path, line, character,
    ))
}

fn lsp_code_action_request_status_for_location(location: &str) -> String {
    format!("Requesting code actions at {location}")
}

#[cfg(test)]
mod tests {
    use super::{lsp_code_action_request_location, lsp_code_action_request_status};
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspCodeAction, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn code_action_request_status_sanitizes_and_bounds_path_label() {
        let path = Path::new("workspace/src").join(format!(
            "bad\n{}\u{202e}actions.rs",
            "very-long-".repeat(32)
        ));

        let location = lsp_code_action_request_location(&path, 2, 4);
        let status = lsp_code_action_request_status(&path, 2, 4);
        let label = display_path_label_cow(&path);

        assert_eq!(location, format!("{label}:3:5"));
        assert_eq!(status, format!("Requesting code actions at {location}"));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        assert!(label.contains("..."));
    }

    #[test]
    fn code_action_request_without_active_target_clears_stale_popup_state() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.code_actions_open = true;
        app.code_actions = vec![action("Fix import")];
        app.code_actions_buffer_id = Some(7);
        app.code_actions_path = Some(path);
        app.code_actions_version = Some(42);
        app.code_actions_line = 9;
        app.code_actions_column = 3;
        app.code_actions_selected = 2;

        app.request_lsp_code_actions();

        assert!(!app.code_actions_open);
        assert!(app.code_actions.is_empty());
        assert_eq!(app.code_actions_buffer_id, None);
        assert_eq!(app.code_actions_path, None);
        assert_eq!(app.code_actions_version, None);
        assert_eq!(app.code_actions_line, 0);
        assert_eq!(app.code_actions_column, 0);
        assert_eq!(app.code_actions_selected, 0);
        assert_eq!(app.status, "No LSP code action target");
    }

    fn action(title: &str) -> LspCodeAction {
        LspCodeAction {
            title: title.to_owned(),
            kind: Some("quickfix".to_owned()),
            edits: Vec::new(),
            document_changes: Vec::new(),
            resolve_payload: None,
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
