mod failed;
mod reloaded;

use crate::file_reload_runtime::FileReloadCompletion;
use crate::{
    KuroyaApp, file_reload_runtime::file_paths_match_lexically, image_preview::LoadedImagePreview,
    ui_events::UiEvent,
};
use kuroya_core::{BufferId, TextBuffer};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct ReloadedFileEvent {
    id: BufferId,
    path: PathBuf,
    buffer: TextBuffer,
    elapsed: Duration,
    version: u64,
    force_dirty: bool,
    lossy: bool,
    binary: bool,
    image_preview: Option<LoadedImagePreview>,
}

impl KuroyaApp {
    pub(crate) fn handle_file_reload_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::FileReloaded {
                root,
                generation,
                request_id,
                id,
                path,
                buffer,
                elapsed,
                version,
                force_dirty,
                lossy,
                binary,
            } => {
                if !self.workspace_event_is_current(&root, generation) {
                    return;
                }
                let completion =
                    self.finish_file_reload_request(request_id, id, &path, version, force_dirty);
                if completion != FileReloadCompletion::Current {
                    return;
                }
                self.apply_file_reloaded_event(ReloadedFileEvent {
                    id,
                    path,
                    buffer,
                    elapsed,
                    version,
                    force_dirty,
                    lossy,
                    binary,
                    image_preview: None,
                });
                self.spawn_queued_reload_after_completion(id);
            }
            UiEvent::ImageFileReloaded {
                root,
                generation,
                request_id,
                id,
                path,
                buffer,
                preview,
                elapsed,
                version,
                force_dirty,
            } => {
                if !self.workspace_event_is_current(&root, generation) {
                    return;
                }
                let completion =
                    self.finish_file_reload_request(request_id, id, &path, version, force_dirty);
                if completion != FileReloadCompletion::Current {
                    return;
                }
                self.apply_file_reloaded_event(ReloadedFileEvent {
                    id,
                    path,
                    buffer,
                    elapsed,
                    version,
                    force_dirty,
                    lossy: false,
                    binary: true,
                    image_preview: Some(preview),
                });
                self.spawn_queued_reload_after_completion(id);
            }
            UiEvent::FileReloadFailed {
                root,
                generation,
                request_id,
                id,
                path,
                error,
                version,
                force_dirty,
            } => {
                if !self.workspace_event_is_current(&root, generation) {
                    return;
                }
                let completion =
                    self.finish_file_reload_request(request_id, id, &path, version, force_dirty);
                if completion != FileReloadCompletion::Current {
                    return;
                }
                self.apply_file_reload_failed_event(id, path, error, version, force_dirty);
                self.spawn_queued_reload_after_completion(id);
            }
            _ => {}
        }
    }

    pub(super) fn file_reload_targets_current_buffer(
        &self,
        id: BufferId,
        path: &Path,
        version: u64,
        force_dirty: bool,
    ) -> bool {
        let Some(buffer) = self.buffer(id) else {
            return false;
        };
        if buffer.version() != version || (buffer.is_dirty() && !force_dirty) {
            return false;
        }
        buffer
            .path()
            .is_some_and(|buffer_path| file_paths_match_lexically(buffer_path, path))
    }
}

#[cfg(test)]
mod tests;
