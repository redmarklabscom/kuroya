use crate::lsp_client::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::{BufferId, LspTypeHierarchyItem};
use std::path::PathBuf;

impl LspClientHandle {
    pub fn prepare_type_hierarchy(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
    ) -> bool {
        self.queue_command(LspClientCommand::PrepareTypeHierarchy {
            id,
            path,
            version,
            line,
            character,
        })
    }

    pub fn type_hierarchy_supertypes(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspTypeHierarchyItem,
    ) -> bool {
        self.queue_command(LspClientCommand::TypeHierarchySupertypes {
            id,
            path,
            version,
            item,
        })
    }

    pub fn type_hierarchy_subtypes(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspTypeHierarchyItem,
    ) -> bool {
        self.queue_command(LspClientCommand::TypeHierarchySubtypes {
            id,
            path,
            version,
            item,
        })
    }
}
