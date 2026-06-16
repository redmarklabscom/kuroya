use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn workspace_symbols(&self, id: BufferId, path: PathBuf, query: String) -> bool {
        self.queue_command(LspClientCommand::WorkspaceSymbols { id, path, query })
    }
}
