use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use std::path::PathBuf;

impl LspClientHandle {
    pub fn did_save(&self, path: PathBuf) -> bool {
        self.queue_command(LspClientCommand::DidSave { path })
    }

    pub fn did_close(&self, path: PathBuf) -> bool {
        self.queue_command(LspClientCommand::DidClose { path })
    }
}
