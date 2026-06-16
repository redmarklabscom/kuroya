use crate::KuroyaApp;
use eframe::egui::Context;

impl KuroyaApp {
    pub(crate) fn render_active_overlays(&mut self, ctx: &Context) {
        if self.quick_open {
            self.render_quick_open(ctx);
        }
        if self.buffer_find_open {
            self.render_buffer_find(ctx);
        }
        if self.goto_line_open {
            self.render_goto_line(ctx);
        }
        if self.workspace_symbols_open {
            self.render_workspace_symbols(ctx);
        }
        if self.workspace_tasks_open {
            self.render_workspace_tasks_panel(ctx);
        }
        if self.lsp_hover.is_some() {
            self.render_lsp_hover(ctx);
        }
        if self.signature_help.is_some() {
            self.render_signature_help(ctx);
        }
        if self.lsp_rename_open {
            self.render_lsp_rename(ctx);
        }
        if self.lsp_rename_preview_open {
            self.render_lsp_rename_preview(ctx);
        }
        if self.completion_open {
            self.render_completion_popup(ctx);
        }
        if self.references_open {
            self.render_references_popup(ctx);
        }
        if self.call_hierarchy_open {
            self.render_call_hierarchy_popup(ctx);
        }
        if self.type_hierarchy_open {
            self.render_type_hierarchy_popup(ctx);
        }
        if self.code_actions_open {
            self.render_code_actions_popup(ctx);
        }
        if self.command_palette {
            self.render_command_palette(ctx);
        }
        if self.source_control_branch_picker_open {
            self.render_git_branch_switcher(ctx);
        }
        if self.source_control_history_open {
            self.render_git_history_panel(ctx);
        }
        if self.source_control_stashes_open {
            self.render_git_stashes_panel(ctx);
        }
        if self.source_control_hunks_open {
            self.render_git_hunks_panel(ctx);
        }
        if self.save_as_open {
            self.render_save_as(ctx);
        }
        if self.settings_panel_open {
            self.render_settings_panel(ctx);
        }
        if self.theme_picker_open {
            self.render_theme_picker(ctx);
        }
        if self.keybindings_open {
            self.render_keybindings_panel(ctx);
        }
        if self.devtools_open {
            self.render_devtools_overlay(ctx);
        }
        if self.gpu_acceleration_prompt.is_some() {
            self.render_gpu_acceleration_prompt(ctx);
        }
        if self.dirty_close_buffer.is_some() {
            self.render_unsaved_close(ctx);
        }
        if self.dirty_reload_buffer.is_some() {
            self.render_reload_from_disk(ctx);
        }
        if self.save_conflict_buffer.is_some() {
            self.render_save_conflict(ctx);
        }
        if self.pending_workspace_switch.is_some() {
            self.render_workspace_switch_guard(ctx);
        }
        if self.pending_exit.is_some() {
            self.render_exit_guard(ctx);
        }
        if self.explorer_file_action.is_some() {
            self.render_explorer_file_action(ctx);
        }
        if self.explorer_delete_target.is_some() {
            self.render_explorer_delete(ctx);
        }
        if self.pending_editor_file_drop.is_some() {
            self.render_editor_file_drop_selector(ctx);
        }
        if self.pending_source_control_discard.is_some() {
            self.render_source_control_discard(ctx);
        }
        if self.pending_source_control_smart_commit.is_some() {
            self.render_source_control_smart_commit(ctx);
        }
        if self.pending_source_control_empty_commit.is_some() {
            self.render_source_control_empty_commit(ctx);
        }
        if self
            .pending_source_control_protected_branch_commit
            .is_some()
        {
            self.render_source_control_protected_branch_commit(ctx);
        }
        if self.pending_source_control_commit_save.is_some() {
            self.render_source_control_commit_save_prompt(ctx);
        }
        if self.pending_source_control_stash_save.is_some() {
            self.render_source_control_stash_save_prompt(ctx);
        }
    }
}
