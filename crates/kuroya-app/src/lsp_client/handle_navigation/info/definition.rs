use crate::lsp_client::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn definition(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
    ) -> bool {
        self.queue_command(LspClientCommand::Definition {
            id,
            path,
            version,
            line,
            character,
        })
    }
}
