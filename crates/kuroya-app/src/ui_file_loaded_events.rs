use crate::{
    KuroyaApp,
    file_runtime::loaded_buffer_path_matches_request,
    image_preview::{ImagePreviewState, LoadedImagePreview},
    path_display::display_path_label_cow,
    persistence::{BufferHistoryState, BufferViewState, PaneBufferViewState},
    transient_state::FileJump,
    workspace_state::{
        PaneId, paths_match_exact_or_lexically, remove_path_map_entry_exact_or_lexically,
        remove_path_set_entry_exact_or_lexically,
    },
};
use kuroya_core::TextBuffer;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

mod existing;
mod first_open;

pub(super) struct LoadedFileTargets {
    pending_jump: Option<FileJump>,
    pending_active: bool,
    pending_panes: Vec<PaneId>,
    pending_view_state: Option<BufferViewState>,
    pending_pane_view_states: Vec<(PaneId, PaneBufferViewState)>,
    pending_history_state: Option<BufferHistoryState>,
}

impl KuroyaApp {
    pub(crate) fn handle_file_loaded(
        &mut self,
        path: PathBuf,
        mut buffer: TextBuffer,
        elapsed: Duration,
        activate: bool,
        lossy: bool,
        binary: bool,
    ) {
        let Some(targets) = self.take_loaded_file_targets(&path) else {
            return;
        };
        if !loaded_buffer_path_matches_request(&buffer, &path) {
            self.status = format!(
                "Could not open {}: loaded buffer path did not match request",
                display_path_label_cow(&path)
            );
            return;
        }
        if buffer.path() != Some(&path) {
            buffer.set_path(path.clone());
        }
        let existing_id = self.buffer_by_lexical_path(&path).map(TextBuffer::id);
        if existing_id.is_none() && self.buffer(buffer.id()).is_some() {
            self.status = format!(
                "Could not open {}: loaded buffer id is already in use",
                display_path_label_cow(&path)
            );
            return;
        }
        if let Some(existing_id) = existing_id {
            self.apply_existing_loaded_file(&path, elapsed, activate, existing_id, targets);
            return;
        }

        self.apply_first_loaded_file(path, buffer, elapsed, activate, lossy, binary, targets);
    }

    pub(crate) fn handle_image_file_loaded(
        &mut self,
        path: PathBuf,
        mut buffer: TextBuffer,
        preview: LoadedImagePreview,
        elapsed: Duration,
        activate: bool,
    ) {
        let Some(targets) = self.take_loaded_file_targets(&path) else {
            return;
        };
        if !loaded_buffer_path_matches_request(&buffer, &path) {
            self.status = format!(
                "Could not open {}: loaded buffer path did not match request",
                display_path_label_cow(&path)
            );
            return;
        }
        if buffer.path() != Some(&path) {
            buffer.set_path(path.clone());
        }
        let existing_id = self.buffer_by_lexical_path(&path).map(TextBuffer::id);
        if existing_id.is_none() && self.buffer(buffer.id()).is_some() {
            self.status = format!(
                "Could not open {}: loaded buffer id is already in use",
                display_path_label_cow(&path)
            );
            return;
        }
        if let Some(existing_id) = existing_id {
            self.image_preview_buffers
                .insert(existing_id, ImagePreviewState::from_loaded(preview));
            self.binary_preview_buffers.insert(existing_id);
            if let Some(buffer) = self.buffer_mut(existing_id) {
                buffer.set_read_only(true);
            }
            self.apply_existing_loaded_file(&path, elapsed, activate, existing_id, targets);
            self.status = format!("Opened {} as image preview", display_path_label_cow(&path));
            return;
        }

        let id = buffer.id();
        self.apply_first_loaded_file(
            path.clone(),
            buffer,
            elapsed,
            activate,
            false,
            true,
            targets,
        );
        if self.buffer(id).is_some() {
            self.image_preview_buffers
                .insert(id, ImagePreviewState::from_loaded(preview));
            self.status = format!("Opened {} as image preview", display_path_label_cow(&path));
        }
    }

    pub(super) fn clear_pending_file_load_state_for_path(&mut self, path: &Path) -> bool {
        self.take_loaded_file_targets(path).is_some()
    }

    fn take_loaded_file_targets(&mut self, path: &Path) -> Option<LoadedFileTargets> {
        if !remove_path_set_entry_exact_or_lexically(&mut self.pending_open_paths, path) {
            return None;
        }
        let pending_jump = self
            .pending_file_jump
            .as_ref()
            .is_some_and(|jump| paths_match_exact_or_lexically(&jump.path, path))
            .then(|| self.pending_file_jump.take())
            .flatten();
        let pending_active = self
            .pending_active_path
            .as_ref()
            .is_some_and(|pending| paths_match_exact_or_lexically(pending, path));
        if pending_active {
            self.pending_active_path = None;
        }
        let pending_panes = self.take_pending_panes_for_path(path);
        let pending_view_state =
            remove_path_map_entry_exact_or_lexically(&mut self.pending_view_states, path);
        let pending_pane_view_states = self.take_pending_pane_view_states_for_path(path);
        let pending_history_state =
            remove_path_map_entry_exact_or_lexically(&mut self.pending_history_states, path);
        Some(LoadedFileTargets {
            pending_jump,
            pending_active,
            pending_panes,
            pending_view_state,
            pending_pane_view_states,
            pending_history_state,
        })
    }

    fn take_pending_pane_view_states_for_path(
        &mut self,
        path: &std::path::Path,
    ) -> Vec<(PaneId, PaneBufferViewState)> {
        let mut pane_ids = self
            .pending_pane_view_states
            .iter()
            .filter_map(|(pane_id, state)| {
                paths_match_exact_or_lexically(&state.path, path).then_some(*pane_id)
            })
            .collect::<Vec<_>>();
        pane_ids.sort_unstable();
        pane_ids
            .into_iter()
            .filter_map(|pane_id| {
                self.pending_pane_view_states
                    .remove(&pane_id)
                    .map(|state| (pane_id, state))
            })
            .collect()
    }
}
