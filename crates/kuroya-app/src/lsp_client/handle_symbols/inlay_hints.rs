use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn inlay_hints(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        end_line: usize,
        end_character: usize,
    ) -> bool {
        self.queue_command(LspClientCommand::InlayHints {
            id,
            path,
            version,
            end_line,
            end_character,
        })
    }
}
