use super::LoadedFileTargets;
use crate::{
    KuroyaApp,
    folding::clamp_folded_ranges_for_line_count,
    large_file_mode::buffer_uses_large_file_mode,
    lsp_lifecycle::buffer_allows_background_language,
    path_display::display_path_label_cow,
    plugin_activation_runtime::{
        activate_plugin_language_for_id, append_plugin_language_activation_status,
        plugin_language_activation_id,
    },
    session_state::{
        apply_buffer_history_state, apply_buffer_view_state,
        horizontal_scroll_offset_from_pane_view_state, horizontal_scroll_offset_from_view_state,
        pane_scroll_line_from_view_state,
    },
    workspace_state::should_activate_loaded_file,
};
use kuroya_core::TextBuffer;
use std::{path::PathBuf, time::Duration};

impl KuroyaApp {
    pub(super) fn apply_first_loaded_file(
        &mut self,
        path: PathBuf,
        mut buffer: TextBuffer,
        elapsed: Duration,
        activate: bool,
        lossy: bool,
        binary: bool,
        mut targets: LoadedFileTargets,
    ) {
        let id = buffer.id();
        clamp_folded_ranges_for_line_count(&mut self.folded_ranges, &path, buffer.len_lines());
        let has_pane_view_states = !targets.pending_pane_view_states.is_empty();
        if targets.pending_jump.is_none()
            && let Some(view_state) = targets.pending_view_state.take()
        {
            let scroll_line = apply_buffer_view_state(&mut buffer, &view_state);
            if !has_pane_view_states {
                if targets.pending_panes.len() > 1 {
                    for pane_id in &targets.pending_panes {
                        self.pending_pane_scroll_lines
                            .insert((*pane_id, id), scroll_line);
                    }
                    let horizontal_scroll_offset =
                        horizontal_scroll_offset_from_view_state(&view_state);
                    if horizontal_scroll_offset > 0.0 {
                        for pane_id in &targets.pending_panes {
                            self.pending_pane_horizontal_scroll_offsets
                                .insert((*pane_id, id), horizontal_scroll_offset);
                        }
                    }
                } else {
                    self.pending_scroll_lines.insert(id, scroll_line);
                    let horizontal_scroll_offset =
                        horizontal_scroll_offset_from_view_state(&view_state);
                    if horizontal_scroll_offset > 0.0 {
                        self.pending_horizontal_scroll_offsets
                            .insert(id, horizontal_scroll_offset);
                    }
                }
            }
        }
        if targets.pending_jump.is_none() {
            for (pane_id, view_state) in &targets.pending_pane_view_states {
                self.pending_pane_scroll_lines.insert(
                    (*pane_id, id),
                    pane_scroll_line_from_view_state(&buffer, view_state),
                );
                let horizontal_scroll_offset =
                    horizontal_scroll_offset_from_pane_view_state(view_state);
                if horizontal_scroll_offset > 0.0 {
                    self.pending_pane_horizontal_scroll_offsets
                        .insert((*pane_id, id), horizontal_scroll_offset);
                }
            }
        }
        if let Some(history_state) = targets.pending_history_state.take() {
            apply_buffer_history_state(&mut buffer, history_state);
        }
        let protected_preview = lossy || binary;
        buffer.set_read_only(protected_preview || self.settings.read_only);
        let hard_large_file_mode = !protected_preview && buffer_uses_large_file_mode(&buffer);
        let background_language_enabled =
            !protected_preview && buffer_allows_background_language(&buffer);
        let plugin_language_id = background_language_enabled
            .then(|| plugin_language_activation_id(&buffer, &self.plugin_languages));
        self.buffers.push(buffer);
        if lossy {
            self.lossy_decoded_buffers.insert(id);
        }
        if binary {
            self.binary_preview_buffers.insert(id);
        }
        let language_activations = activate_plugin_language_for_id(
            &mut self.plugin_activations,
            &self.plugin_runtimes,
            plugin_language_id.as_deref(),
        );
        let path_label = display_path_label_cow(&path);
        if background_language_enabled {
            self.spawn_diagnostics_for(id);
            self.notify_lsp_open(id);
        } else {
            self.diagnostics.replace_static(path.clone(), Vec::new());
        }
        for pane_id in &targets.pending_panes {
            self.assign_buffer_to_pane(*pane_id, id);
        }
        if targets.pending_active {
            let pane_id = self
                .active_pane_holding_buffer(id)
                .or_else(|| self.pane_id_for_buffer(id))
                .or_else(|| targets.pending_panes.first().copied())
                .unwrap_or(self.active_pane);
            self.set_active_buffer_in_pane(pane_id, id);
            self.pending_active_path = None;
        } else if self.active.is_none()
            && self.pending_active_path.is_none()
            && !targets.pending_panes.is_empty()
        {
            self.set_active_buffer_in_pane(targets.pending_panes[0], id);
        } else if should_activate_loaded_file(
            activate,
            targets.pending_jump.is_some(),
            self.active.is_some(),
            self.pending_active_path.is_some(),
        ) {
            self.set_active_buffer(id);
        }
        let mode = match (
            binary,
            lossy,
            background_language_enabled,
            hard_large_file_mode,
        ) {
            (true, _, _, _) => " (binary preview mode)",
            (false, true, _, _) => " (UTF-8 replacement mode)",
            (false, false, true, _) => "",
            (false, false, false, true) => " (large file mode)",
            (false, false, false, false) => "",
        };
        let decode_note = if binary && lossy {
            " with binary/UTF-8 replacement preview"
        } else if binary {
            " with binary preview"
        } else if lossy {
            " with UTF-8 replacements"
        } else {
            ""
        };
        if let Some(jump) = targets.pending_jump {
            self.apply_file_jump_with_encoding(id, jump.line, jump.column, jump.column_encoding);
            self.status = append_plugin_language_activation_status(
                format!(
                    "Opened {} at {}:{} in {:.1?}{mode}{decode_note}",
                    path_label, jump.line, jump.column, elapsed
                ),
                &language_activations,
            );
        } else {
            self.status = append_plugin_language_activation_status(
                format!(
                    "Opened {} in {:.1?}{mode}{decode_note}",
                    path_label, elapsed
                ),
                &language_activations,
            );
        }
    }
}
