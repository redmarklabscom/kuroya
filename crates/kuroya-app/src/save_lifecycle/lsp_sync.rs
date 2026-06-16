use kuroya_core::TextBuffer;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LspSaveSyncPlan {
    pub(crate) open: bool,
    pub(crate) change: bool,
    pub(crate) save: bool,
    pub(crate) reschedule: bool,
}

pub(crate) fn plan_lsp_save_sync(
    path_changed: bool,
    had_pending_sync: bool,
    still_dirty: bool,
) -> LspSaveSyncPlan {
    if path_changed {
        return LspSaveSyncPlan {
            open: true,
            save: !still_dirty,
            ..LspSaveSyncPlan::default()
        };
    }

    if still_dirty {
        return LspSaveSyncPlan {
            reschedule: had_pending_sync,
            ..LspSaveSyncPlan::default()
        };
    }

    LspSaveSyncPlan {
        change: had_pending_sync,
        save: true,
        ..LspSaveSyncPlan::default()
    }
}

pub(crate) fn apply_save_completion(
    buffer: &mut TextBuffer,
    path: PathBuf,
    saved_version: u64,
) -> bool {
    buffer.set_path(path);
    if buffer.version() == saved_version {
        buffer.mark_saved();
    }
    buffer.is_dirty()
}

#[cfg(test)]
mod tests {
    use super::{LspSaveSyncPlan, apply_save_completion, plan_lsp_save_sync};
    use kuroya_core::TextBuffer;
    use std::path::PathBuf;

    #[test]
    fn save_completion_preserves_raw_save_target_path_for_newer_edits() {
        let raw_path = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("saved.rs");
        let mut buffer = TextBuffer::from_text(7, None, "saved text".to_owned());
        buffer.mark_dirty();
        let saved_version = buffer.version();
        buffer.insert_at_cursor("newer ");

        assert!(apply_save_completion(
            &mut buffer,
            raw_path.clone(),
            saved_version
        ));

        assert_eq!(buffer.path(), Some(&raw_path));
        assert!(buffer.is_dirty());
    }

    #[test]
    fn stale_large_buffer_save_completion_keeps_newer_edits_dirty() {
        let path = PathBuf::from("workspace/large.txt");
        let mut buffer = TextBuffer::from_text(7, None, "x".repeat(2 * 1024 * 1024 + 1));
        buffer.mark_dirty();
        let saved_version = buffer.version();
        buffer.insert_at_cursor("newer");

        assert!(apply_save_completion(
            &mut buffer,
            path.clone(),
            saved_version
        ));

        assert_eq!(buffer.path(), Some(&path));
        assert!(buffer.is_dirty());
    }

    #[test]
    fn future_version_save_completion_does_not_clear_dirty_state() {
        let path = PathBuf::from("workspace/src/main.rs");
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        let future_version = buffer.version().saturating_add(1);

        assert!(apply_save_completion(
            &mut buffer,
            path.clone(),
            future_version
        ));

        assert_eq!(buffer.path(), Some(&path));
        assert!(buffer.is_dirty());
    }

    #[test]
    fn path_changed_clean_save_opens_before_sending_lsp_save() {
        assert_eq!(
            plan_lsp_save_sync(true, true, false),
            LspSaveSyncPlan {
                open: true,
                save: true,
                ..LspSaveSyncPlan::default()
            }
        );
    }

    #[test]
    fn path_changed_dirty_save_does_not_send_false_lsp_save() {
        assert_eq!(
            plan_lsp_save_sync(true, false, true),
            LspSaveSyncPlan {
                open: true,
                ..LspSaveSyncPlan::default()
            }
        );
    }
}
