mod state;

use super::ReloadedFileEvent;
use crate::image_preview::{ImagePreviewState, enforce_image_preview_retained_byte_cap};
use crate::{
    KuroyaApp, file_reload_runtime::file_paths_match_lexically,
    file_runtime::loaded_buffer_path_matches_request, path_display::display_path_label_cow,
};
use state::reload_decode_note;

impl KuroyaApp {
    pub(super) fn apply_file_reloaded_event(&mut self, event: ReloadedFileEvent) {
        if !loaded_buffer_path_matches_request(&event.buffer, &event.path) {
            self.status = format!(
                "Could not reload {}: loaded buffer path did not match request",
                display_path_label_cow(&event.path)
            );
            self.mark_unapplied_file_reload_as_external_change(
                event.id,
                &event.path,
                event.force_dirty,
            );
            return;
        }
        if self.can_apply_file_reload(event.id, &event.path, event.version, event.force_dirty) {
            let is_image_preview = event.image_preview.is_some();
            if let Some(preview) = event.image_preview {
                self.image_preview_buffers
                    .insert(event.id, ImagePreviewState::from_loaded(preview));
                let keep_ids = self.active.into_iter().chain(std::iter::once(event.id));
                enforce_image_preview_retained_byte_cap(&mut self.image_preview_buffers, keep_ids);
            } else {
                self.image_preview_buffers.remove(&event.id);
            }
            self.apply_reload_preview_markers(event.id, event.lossy, event.binary);
            let protected_preview = event.lossy || event.binary;
            if protected_preview {
                self.invalidate_static_diagnostics_request(event.id);
                self.diagnostics
                    .replace_static(event.path.clone(), Vec::new());
            }
            let text_matches = self.buffer(event.id).is_some_and(|buffer| {
                !(event.force_dirty && buffer.is_dirty())
                    && buffer.text_equals_buffer(&event.buffer)
            });
            if text_matches {
                self.clear_reload_conflict_state(event.id);
                if event.force_dirty {
                    self.status = format!(
                        "{} is already up to date",
                        display_path_label_cow(&event.path)
                    );
                }
                return;
            }
            if let Some(buffer) = self.buffer_mut(event.id) {
                buffer.replace_from_disk_buffer(event.buffer);
            }
            self.clear_folding_state_for_path(&event.path);
            self.diff_cache.remove(&event.id);
            self.clear_buffer_merge_conflict_cache(event.id);
            self.clear_reload_conflict_state(event.id);
            self.pending_language_sync.remove(&event.id);
            if !protected_preview {
                self.spawn_diagnostics_for(event.id);
                self.notify_lsp_change(event.id);
            }
            if is_image_preview {
                self.status = format!(
                    "Reloaded {} as image preview",
                    display_path_label_cow(&event.path)
                );
            } else {
                self.status = format!(
                    "Reloaded {} in {:.1?}{}",
                    display_path_label_cow(&event.path),
                    event.elapsed,
                    reload_decode_note(event.lossy, event.binary)
                );
            }
        } else if event.force_dirty
            && self.buffer(event.id).is_some_and(|buffer| {
                buffer
                    .path()
                    .is_some_and(|path| file_paths_match_lexically(path, &event.path))
            })
        {
            self.dirty_reload_buffer = Some(event.id);
            self.status = format!(
                "Skipped reload for {} because it changed locally",
                display_path_label_cow(&event.path)
            );
        } else {
            self.mark_unapplied_file_reload_as_external_change(
                event.id,
                &event.path,
                event.force_dirty,
            );
        }
    }
}
