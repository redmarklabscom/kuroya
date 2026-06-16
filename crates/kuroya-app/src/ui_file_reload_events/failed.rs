use crate::{
    KuroyaApp,
    path_display::{display_error_label_cow, display_path_label_cow},
};
use kuroya_core::BufferId;
use std::path::PathBuf;

impl KuroyaApp {
    pub(super) fn apply_file_reload_failed_event(
        &mut self,
        id: BufferId,
        path: PathBuf,
        error: String,
        version: u64,
        force_dirty: bool,
    ) {
        if self.file_reload_targets_current_buffer(id, &path, version, force_dirty) {
            self.status = format!(
                "Could not reload {}: {}",
                display_path_label_cow(&path),
                display_error_label_cow(&error)
            );
        }
        self.mark_unapplied_file_reload_as_external_change(id, &path, force_dirty);
    }
}
