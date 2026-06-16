use crate::{KuroyaApp, lsp_runtime::lsp_command_queue_failed_status};

use super::info::lsp_request_location_label;

impl KuroyaApp {
    pub(crate) fn request_lsp_type_hierarchy(&mut self) {
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.clear_type_hierarchy();
            self.status = "No LSP type hierarchy target".to_owned();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.clear_type_hierarchy();
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        if !client.prepare_type_hierarchy(id, path.clone(), version, line, character) {
            self.clear_type_hierarchy();
            self.status = lsp_command_queue_failed_status("textDocument/prepareTypeHierarchy");
            return;
        }
        let location_label = lsp_request_location_label(&path, line, character);
        self.record_lsp_client_trace("textDocument/prepareTypeHierarchy", location_label.clone());
        self.type_hierarchy_open = true;
        self.type_hierarchy_root = None;
        self.type_hierarchy_supertypes.clear();
        self.type_hierarchy_subtypes.clear();
        self.type_hierarchy_selected = 0;
        self.type_hierarchy_path = Some(path);
        self.type_hierarchy_line = line + 1;
        self.type_hierarchy_column = character + 1;
        self.completion_open = false;
        self.code_actions_open = false;
        self.signature_help = None;
        self.lsp_hover = None;
        self.status = format!("Requesting type hierarchy at {location_label}");
    }

    pub(crate) fn clear_type_hierarchy(&mut self) {
        self.type_hierarchy_open = false;
        self.type_hierarchy_root = None;
        self.type_hierarchy_supertypes.clear();
        self.type_hierarchy_subtypes.clear();
        self.type_hierarchy_selected = 0;
        self.type_hierarchy_path = None;
        self.type_hierarchy_line = 0;
        self.type_hierarchy_column = 0;
    }
}
