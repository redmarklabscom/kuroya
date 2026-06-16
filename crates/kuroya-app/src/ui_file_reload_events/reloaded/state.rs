use crate::KuroyaApp;
use kuroya_core::{BufferId, TextBuffer};
use std::path::Path;

impl KuroyaApp {
    pub(super) fn can_apply_file_reload(
        &self,
        id: BufferId,
        path: &Path,
        version: u64,
        force_dirty: bool,
    ) -> bool {
        self.file_reload_targets_current_buffer(id, path, version, force_dirty)
    }

    pub(super) fn apply_reload_preview_markers(&mut self, id: BufferId, lossy: bool, binary: bool) {
        let was_protected_preview =
            self.lossy_decoded_buffers.contains(&id) || self.binary_preview_buffers.contains(&id);
        let was_read_only = self.buffer(id).is_some_and(TextBuffer::is_read_only);
        let manual_read_only = self.manual_read_only_buffers.contains(&id);
        let read_only = reload_read_only_state(
            was_read_only,
            was_protected_preview,
            manual_read_only,
            lossy,
            binary,
            self.settings.read_only,
        );
        if lossy {
            self.lossy_decoded_buffers.insert(id);
        } else {
            self.lossy_decoded_buffers.remove(&id);
        }
        if binary {
            self.binary_preview_buffers.insert(id);
        } else {
            self.binary_preview_buffers.remove(&id);
        }
        if let Some(buffer) = self.buffer_mut(id) {
            buffer.set_read_only(read_only);
        }
    }

    pub(super) fn clear_reload_conflict_state(&mut self, id: BufferId) {
        self.clear_buffer_changed_on_disk(id);
        self.dirty_reload_buffer = self.dirty_reload_buffer.filter(|dirty| *dirty != id);
        self.save_conflict_buffer = self.save_conflict_buffer.filter(|conflict| *conflict != id);
    }
}

pub(super) fn reload_decode_note(lossy: bool, binary: bool) -> &'static str {
    if binary && lossy {
        " with binary/UTF-8 replacement preview"
    } else if binary {
        " with binary preview"
    } else if lossy {
        " with UTF-8 replacements"
    } else {
        ""
    }
}

fn reload_read_only_state(
    was_read_only: bool,
    was_protected_preview: bool,
    manual_read_only: bool,
    lossy: bool,
    binary: bool,
    global_read_only: bool,
) -> bool {
    let protected_preview = lossy || binary;
    protected_preview
        || global_read_only
        || manual_read_only
        || (was_read_only && !was_protected_preview)
}

#[cfg(test)]
mod tests {
    use super::reload_read_only_state;

    #[test]
    fn reload_preserves_manual_read_only_for_clean_text() {
        assert!(reload_read_only_state(
            true, false, true, false, false, false
        ));
    }

    #[test]
    fn reload_clears_preview_only_read_only_when_decode_becomes_safe() {
        assert!(!reload_read_only_state(
            true, true, false, false, false, false
        ));
    }

    #[test]
    fn reload_preserves_manual_read_only_after_protected_preview_cycle() {
        assert!(reload_read_only_state(
            true, true, true, false, false, false
        ));
    }

    #[test]
    fn reload_forces_read_only_for_new_protected_or_global_states() {
        assert!(reload_read_only_state(
            false, false, false, true, false, false
        ));
        assert!(reload_read_only_state(
            false, false, false, false, true, false
        ));
        assert!(reload_read_only_state(
            false, false, false, false, false, true
        ));
    }
}
