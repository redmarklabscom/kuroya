use crate::{KuroyaApp, lsp_rename_requests::lsp_rename_prefill_target};

impl KuroyaApp {
    pub(crate) fn begin_lsp_rename(&mut self) {
        let Some((_, _, _, _, _)) = self.active_lsp_position() else {
            self.lsp_rename_open = false;
            self.lsp_rename_input.clear();
            self.clear_lsp_rename_preview_state();
            self.status = "No LSP rename target".to_owned();
            return;
        };

        self.clear_lsp_rename_preview_state();
        self.lsp_rename_input = self
            .active_buffer()
            .and_then(|buffer| {
                buffer
                    .selected_text()
                    .filter(|text| !text.contains('\n'))
                    .or_else(|| buffer.word_at_cursor())
            })
            .and_then(|text| lsp_rename_prefill_target(&text))
            .unwrap_or_default();
        self.lsp_rename_open = true;
        self.status = "Rename symbol".to_owned();
    }
}

#[cfg(test)]
mod tests {
    use crate::{KuroyaApp, app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, LspTextEdit, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn begin_lsp_rename_closes_stale_popup_when_no_active_target() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        app.lsp_rename_open = true;
        app.lsp_rename_input = "stale_name".to_owned();
        app.lsp_rename_preview_open = true;
        app.lsp_rename_preview_new_name = "stale".to_owned();
        app.lsp_rename_preview_edits = vec![text_edit(root.join("src/main.rs"))];

        app.begin_lsp_rename();

        assert!(!app.lsp_rename_open);
        assert!(app.lsp_rename_input.is_empty());
        assert!(!app.lsp_rename_preview_open);
        assert!(app.lsp_rename_preview_edits.is_empty());
        assert_eq!(app.status, "No LSP rename target");
    }

    fn text_edit(path: PathBuf) -> LspTextEdit {
        LspTextEdit {
            path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 5,
            new_text: "renamed".to_owned(),
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
