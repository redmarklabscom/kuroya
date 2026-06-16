use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn document_symbols(&self, id: BufferId, path: PathBuf, version: u64) -> bool {
        self.queue_command(LspClientCommand::DocumentSymbols { id, path, version })
    }
}
