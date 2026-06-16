use crate::{
    KuroyaApp,
    lsp_rename_requests::{
        lsp_rename_display_label, lsp_rename_request_target, lsp_rename_target_error_status,
    },
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::compact_path,
    ui_text::truncate_middle,
};

const LSP_RENAME_SUBMIT_LABEL_MAX_CHARS: usize = 64;

impl KuroyaApp {
    pub(crate) fn submit_lsp_rename(&mut self) {
        let new_name = match lsp_rename_request_target(&self.lsp_rename_input) {
            Ok(new_name) => new_name,
            Err(error) => {
                self.status = lsp_rename_target_error_status(error);
                return;
            }
        };
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.lsp_rename_open = false;
            self.status = "No LSP rename target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.rename(id, path.clone(), version, line, character, new_name.clone()) {
            self.status = lsp_command_queue_failed_status("textDocument/rename");
            return;
        }
        let new_name_label = lsp_rename_submit_label(&new_name);
        let path_label = lsp_rename_submit_label(&compact_path(&path));
        let display_line = line.saturating_add(1);
        let display_character = character.saturating_add(1);
        self.record_lsp_client_trace(
            "textDocument/rename",
            format!(
                "{path_label}:{}:{} `{new_name_label}`",
                display_line, display_character
            ),
        );
        self.lsp_rename_open = false;
        self.status = format!(
            "Requesting rename at {path_label}:{}:{} to `{new_name_label}`",
            display_line, display_character
        );
    }
}

fn lsp_rename_submit_label(text: &str) -> String {
    truncate_middle(
        &lsp_rename_display_label(text),
        LSP_RENAME_SUBMIT_LABEL_MAX_CHARS,
    )
}

#[cfg(test)]
mod tests {
    use super::{LSP_RENAME_SUBMIT_LABEL_MAX_CHARS, lsp_rename_submit_label};
    use crate::{KuroyaApp, app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn submit_lsp_rename_closes_stale_popup_when_no_active_target() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        app.lsp_rename_open = true;
        app.lsp_rename_input = "renamed_symbol".to_owned();
        app.status = "unchanged".to_owned();

        app.submit_lsp_rename();

        assert!(!app.lsp_rename_open);
        assert_eq!(app.lsp_rename_input, "renamed_symbol");
        assert_eq!(app.status, "No LSP rename target");
    }

    #[test]
    fn rename_submit_label_escapes_and_bounds_display_text() {
        let raw = format!(
            "path\n{}\t\u{202e}tail",
            "segment-".repeat(LSP_RENAME_SUBMIT_LABEL_MAX_CHARS)
        );
        let label = lsp_rename_submit_label(&raw);

        assert!(raw.contains('\n'));
        assert!(raw.contains('\u{202e}'));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\t'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= LSP_RENAME_SUBMIT_LABEL_MAX_CHARS);
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
