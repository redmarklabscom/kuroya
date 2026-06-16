use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::{BufferId, LspCodeLens};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

impl LspClientHandle {
    pub fn code_lenses(&self, id: BufferId, path: PathBuf, version: u64) -> bool {
        self.queue_command(LspClientCommand::CodeLenses { id, path, version })
    }

    pub fn resolve_code_lens(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        lens: LspCodeLens,
    ) -> bool {
        if !lens.needs_resolve() {
            return false;
        }
        self.queue_command(LspClientCommand::ResolveCodeLens {
            id,
            path,
            version,
            lens,
        })
    }

    pub fn execute_command(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        title: String,
        command: String,
        arguments: Option<Arc<Value>>,
    ) -> bool {
        if command.trim().is_empty() {
            return false;
        }
        self.queue_command(LspClientCommand::ExecuteCommand {
            id,
            path,
            version,
            title,
            command,
            arguments,
        })
    }
}
