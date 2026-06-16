use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn folding_ranges(&self, id: BufferId, path: PathBuf, version: u64) -> bool {
        self.queue_command(LspClientCommand::FoldingRanges { id, path, version })
    }
}
