use crate::{KuroyaApp, session_state::EditorPane};

impl KuroyaApp {
    pub(super) fn reset_workspace_save_and_pane_state(&mut self) {
        self.dirty_close_buffer = None;
        self.dirty_reload_buffer = None;
        self.save_conflict_buffer = None;
        self.pending_close_buffers.clear();
        self.close_after_save = None;
        self.save_as_open = false;
        self.save_as_buffer = None;
        self.save_as_path.clear();
        self.in_flight_saves.clear();
        self.queued_save_paths.clear();
        self.pending_format_on_save.clear();
        self.pending_format_on_save_started.clear();
        self.pending_format_on_save_retries.clear();
        self.canceled_formatting_request_ids.clear();
        self.canceled_formatting_request_order.clear();
        self.clear_format_on_save_overwrite_external_changes();
        self.format_on_save_bypass.clear();
        self.in_flight_reloads.clear();
        self.queued_file_reloads.clear();
        self.canceled_file_reloads.clear();
        self.canceled_file_reload_order.clear();
        self.pending_open_paths.clear();
        self.pending_pane_paths.clear();
        self.pending_view_states.clear();
        self.pending_pane_view_states.clear();
        self.pending_history_states.clear();
        self.panes.clear();
        self.panes.push(EditorPane {
            id: 1,
            active: None,
            weight: 1.0,
        });
        self.active_pane = 1;
        self.focused_pane = None;
        self.last_autosave_focused_pane = None;
        self.next_pane_id = 2;
        self.active = None;
        self.pending_active_path = None;
        self.pending_file_jump = None;
        self.navigation_back.clear();
        self.navigation_forward.clear();
        self.closed_files.clear();
        self.pending_scroll_lines.clear();
        self.pending_horizontal_scroll_offsets.clear();
        self.pending_pane_scroll_lines.clear();
        self.pending_pane_horizontal_scroll_offsets.clear();
        self.editor_scroll_offsets.clear();
        self.editor_horizontal_scroll_offsets.clear();
        self.editor_scroll_targets.clear();
        self.editor_inertial_scrolls.clear();
        self.editor_middle_click_scroll = None;
        self.editor_selection_clipboard = None;
        self.pending_language_sync.clear();
        self.pending_lsp_symbol_refreshes.clear();
        self.pending_signature_help_requests.clear();
        self.pending_format_on_type_requests.clear();
    }
}
