use crate::{
    KuroyaApp,
    editor_input::{EditorContextAction, editor_context_action_edits_buffer},
};
use eframe::egui::Context;
use kuroya_core::BufferId;

impl KuroyaApp {
    pub(crate) fn run_editor_context_action(
        &mut self,
        ctx: &Context,
        buffer_id: BufferId,
        action: EditorContextAction,
    ) {
        if editor_context_action_edits_buffer(action)
            && self.block_protected_preview_edit(buffer_id)
        {
            return;
        }
        if self.run_editor_lsp_context_action(buffer_id, action) {
            return;
        }
        if self.run_editor_clipboard_context_action(ctx, buffer_id, action) {
            return;
        }
        self.run_editor_buffer_context_action(buffer_id, action);
    }
}
