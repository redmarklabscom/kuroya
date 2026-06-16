use crate::{
    lsp_client::{commands::LspClientCommand, handle::LspClientHandle},
    lsp_completion_resolve::CompletionResolveIntent,
};
use kuroya_core::{BufferId, LspCompletionItem};
use std::path::PathBuf;

impl LspClientHandle {
    pub fn completion(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
    ) -> bool {
        self.queue_command(LspClientCommand::Completion {
            id,
            path,
            version,
            line,
            character,
        })
    }

    pub fn resolve_completion_item(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        character: usize,
        item: LspCompletionItem,
        intent: CompletionResolveIntent,
    ) -> bool {
        self.queue_command(LspClientCommand::ResolveCompletionItem {
            id,
            path,
            version,
            line,
            character,
            item: Box::new(item),
            intent,
        })
    }
}
