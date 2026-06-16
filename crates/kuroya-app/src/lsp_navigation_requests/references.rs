use crate::{KuroyaApp, lsp_runtime::lsp_command_queue_failed_status};

use super::info::lsp_request_location_label;

impl KuroyaApp {
    pub(crate) fn request_lsp_references(&mut self) {
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.references_open = false;
            self.references.clear();
            self.status = "No LSP references target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.references_open = false;
            self.references.clear();
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.references(id, path.clone(), version, line, character, true) {
            self.references_open = false;
            self.references.clear();
            self.status = lsp_command_queue_failed_status("textDocument/references");
            return;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.record_lsp_client_trace("textDocument/references", location_label.clone());
        self.references_open = true;
        self.references.clear();
        self.references_path = Some(path);
        self.references_line = line + 1;
        self.references_column = character + 1;
        self.references_selected = 0;
        self.completion_open = false;
        self.code_actions_open = false;
        self.signature_help = None;
        self.lsp_hover = None;
        self.status = format!("Requesting references at {location_label}");
    }
}
