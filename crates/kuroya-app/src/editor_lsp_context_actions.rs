use crate::{KuroyaApp, editor_input::EditorContextAction};
use kuroya_core::BufferId;

impl KuroyaApp {
    pub(crate) fn run_editor_lsp_context_action(
        &mut self,
        buffer_id: BufferId,
        action: EditorContextAction,
    ) -> bool {
        if !is_editor_lsp_context_action(action) {
            return false;
        }

        if self.buffer(buffer_id).is_none() {
            self.status = "Editor action target unavailable".to_owned();
            return false;
        }

        self.set_active_buffer(buffer_id);
        match action {
            EditorContextAction::ShowHover => {
                self.request_lsp_hover();
            }
            EditorContextAction::DocumentHighlights => {
                self.request_lsp_document_highlights();
            }
            EditorContextAction::GoToDefinition => {
                self.request_lsp_definition();
            }
            EditorContextAction::FindReferences => {
                self.request_lsp_references();
            }
            EditorContextAction::ShowCallHierarchy => {
                self.request_lsp_call_hierarchy();
            }
            EditorContextAction::ShowTypeHierarchy => {
                self.request_lsp_type_hierarchy();
            }
            EditorContextAction::RenameSymbol => {
                self.begin_lsp_rename();
            }
            EditorContextAction::ShowSymbols => {
                self.symbols_panel = true;
                self.request_lsp_document_symbols();
            }
            EditorContextAction::WorkspaceSymbols => {
                self.begin_workspace_symbols();
            }
            EditorContextAction::ShowCompletions => {
                self.request_lsp_completion();
            }
            EditorContextAction::SignatureHelp => {
                self.request_lsp_signature_help();
            }
            EditorContextAction::LoadFolds => {
                self.request_lsp_folding_ranges();
            }
            EditorContextAction::ToggleFold => {
                self.toggle_fold_at_cursor();
            }
            EditorContextAction::ExpandAllFolds => {
                self.expand_all_folds();
            }
            EditorContextAction::FormatDocument => {
                self.request_lsp_formatting();
            }
            EditorContextAction::CodeActions => {
                self.request_lsp_code_actions();
            }
            _ => unreachable!("action is checked by is_editor_lsp_context_action"),
        }

        true
    }
}

fn is_editor_lsp_context_action(action: EditorContextAction) -> bool {
    matches!(
        action,
        EditorContextAction::ShowHover
            | EditorContextAction::DocumentHighlights
            | EditorContextAction::GoToDefinition
            | EditorContextAction::FindReferences
            | EditorContextAction::ShowCallHierarchy
            | EditorContextAction::ShowTypeHierarchy
            | EditorContextAction::RenameSymbol
            | EditorContextAction::ShowSymbols
            | EditorContextAction::WorkspaceSymbols
            | EditorContextAction::ShowCompletions
            | EditorContextAction::SignatureHelp
            | EditorContextAction::LoadFolds
            | EditorContextAction::ToggleFold
            | EditorContextAction::ExpandAllFolds
            | EditorContextAction::FormatDocument
            | EditorContextAction::CodeActions
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn lsp_context_action_rejects_stale_buffer_without_changing_active_buffer() {
        let root = missing_path("workspace-root");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(root.join("src").join("main.rs")),
            "fn main() {}\n".to_owned(),
        ));
        app.set_active_buffer(1);

        let handled = app.run_editor_lsp_context_action(99, EditorContextAction::ShowHover);

        assert!(!handled);
        assert_eq!(app.active, Some(1));
        assert_eq!(app.status, "Editor action target unavailable");
    }

    #[test]
    fn non_lsp_context_action_is_not_handled() {
        let root = missing_path("workspace-root");
        let mut app = app_for_test(root);
        let handled = app.run_editor_lsp_context_action(99, EditorContextAction::Copy);

        assert!(!handled);
        assert_ne!(app.status, "Editor action target unavailable");
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

    fn missing_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kuroya-editor-lsp-context-action-{}-{unique}-{name}",
            std::process::id()
        ))
    }
}
