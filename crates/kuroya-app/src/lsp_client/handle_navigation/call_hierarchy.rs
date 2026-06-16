use crate::lsp_client::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::{BufferId, LspCallHierarchyItem};
use std::path::PathBuf;

impl LspClientHandle {
    pub fn prepare_call_hierarchy(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
    ) -> bool {
        self.queue_command(LspClientCommand::PrepareCallHierarchy {
            id,
            path,
            version,
            line,
            character,
        })
    }

    pub fn call_hierarchy_incoming(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspCallHierarchyItem,
    ) -> bool {
        self.queue_command(LspClientCommand::CallHierarchyIncoming {
            id,
            path,
            version,
            item,
        })
    }

    pub fn call_hierarchy_outgoing(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspCallHierarchyItem,
    ) -> bool {
        self.queue_command(LspClientCommand::CallHierarchyOutgoing {
            id,
            path,
            version,
            item,
        })
    }
}
