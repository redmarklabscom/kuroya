use crate::lsp_client::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl LspClientHandle {
    pub fn formatting(
        &self,
        request_id: u64,
        id: BufferId,
        path: PathBuf,
        version: u64,
        tab_size: usize,
        insert_spaces: bool,
    ) -> bool {
        self.queue_command(LspClientCommand::Formatting {
            request_id,
            id,
            path,
            version,
            tab_size,
            insert_spaces,
        })
    }
}
