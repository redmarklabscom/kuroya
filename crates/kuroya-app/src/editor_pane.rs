use crate::{
    KuroyaApp, editor_text_geometry::measured_monospace_char_width, workspace_state::PaneId,
};
use eframe::egui;
use kuroya_core::BufferId;

mod viewport;

impl KuroyaApp {
    pub(crate) fn render_editor_pane(
        &mut self,
        ui: &mut egui::Ui,
        pane_id: PaneId,
        active_id: Option<BufferId>,
    ) {
        let Some(active_id) = active_id else {
            self.render_empty_editor_pane(ui);
            return;
        };

        let Some(buffer_index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.id() == active_id)
        else {
            return;
        };

        if self.focused_pane.is_none() && self.active_pane == pane_id {
            self.focused_pane = Some(pane_id);
        }

        self.handle_editor_input(ui.ctx(), pane_id, active_id);

        let char_width = measured_monospace_char_width(ui, self.settings.font_size);
        let is_focused = self.focused_pane == Some(pane_id);
        let accepts_text_input = self.editor_accepts_text_input(ui.ctx(), pane_id);
        let data = self.prepare_editor_pane_data(
            active_id,
            buffer_index,
            char_width,
            is_focused,
            accepts_text_input,
        );
        let diff_source_file_actions = data.diff_source_file_actions;
        self.render_editor_pane_header(
            ui,
            pane_id,
            active_id,
            data.active_path.as_deref(),
            diff_source_file_actions,
        );

        let pending_actions =
            self.render_editor_pane_viewport(ui, pane_id, active_id, buffer_index, data);
        self.apply_editor_pane_actions(ui.ctx(), pane_id, active_id, pending_actions);
    }
}
