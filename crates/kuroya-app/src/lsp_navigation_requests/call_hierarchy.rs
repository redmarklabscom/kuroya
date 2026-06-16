use crate::{KuroyaApp, lsp_runtime::lsp_command_queue_failed_status};

use super::info::lsp_request_location_label;

impl KuroyaApp {
    pub(crate) fn request_lsp_call_hierarchy(&mut self) {
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.clear_call_hierarchy();
            self.status = "No LSP call hierarchy target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.clear_call_hierarchy();
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.prepare_call_hierarchy(id, path.clone(), version, line, character) {
            self.clear_call_hierarchy();
            self.status = lsp_command_queue_failed_status("textDocument/prepareCallHierarchy");
            return;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.record_lsp_client_trace("textDocument/prepareCallHierarchy", location_label.clone());
        self.call_hierarchy_open = true;
        self.call_hierarchy_root = None;
        self.call_hierarchy_incoming.clear();
        self.call_hierarchy_outgoing.clear();
        self.call_hierarchy_selected = 0;
        self.call_hierarchy_path = Some(path);
        self.call_hierarchy_line = line + 1;
        self.call_hierarchy_column = character + 1;
        self.completion_open = false;
        self.code_actions_open = false;
        self.signature_help = None;
        self.lsp_hover = None;
        self.status = format!("Requesting call hierarchy at {location_label}");
    }

    pub(crate) fn clear_call_hierarchy(&mut self) {
        self.call_hierarchy_open = false;
        self.call_hierarchy_root = None;
        self.call_hierarchy_incoming.clear();
        self.call_hierarchy_outgoing.clear();
        self.call_hierarchy_selected = 0;
        self.call_hierarchy_path = None;
        self.call_hierarchy_line = 0;
        self.call_hierarchy_column = 0;
    }
}
