use crate::lsp_client::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::{BufferId, Diagnostic, LspCodeAction};
use std::path::PathBuf;

impl LspClientHandle {
    pub fn code_actions(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        origin_line: usize,
        origin_character: usize,
        start_line: usize,
        start_character: usize,
        end_line: usize,
        end_character: usize,
        diagnostics: Vec<Diagnostic>,
    ) -> bool {
        self.queue_command(LspClientCommand::CodeActions {
            id,
            path,
            version,
            origin_line,
            origin_character,
            start_line,
            start_character,
            end_line,
            end_character,
            diagnostics,
        })
    }

    pub fn resolve_code_action(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
        action: LspCodeAction,
    ) -> bool {
        self.queue_command(LspClientCommand::ResolveCodeAction {
            id,
            path,
            version,
            line,
            character,
            action,
        })
    }
}
