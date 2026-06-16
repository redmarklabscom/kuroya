use crate::lsp_client::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn references(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
        include_declaration: bool,
    ) -> bool {
        self.queue_command(LspClientCommand::References {
            id,
            path,
            version,
            line,
            character,
            include_declaration,
        })
    }
}
