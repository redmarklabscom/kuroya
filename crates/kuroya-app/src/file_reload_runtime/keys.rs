use crate::{
    app_state::{PendingFileReload, QueuedFileReload},
    workspace_trust::trusted_workspace_paths_match,
};
use kuroya_core::BufferId;
use std::path::Path;

pub(crate) fn file_paths_match_lexically(left: &Path, right: &Path) -> bool {
    trusted_workspace_paths_match(left, right)
}

pub(super) struct FileReloadCompletionKey<'a> {
    pub(super) request_id: u64,
    pub(super) path: &'a Path,
    pub(super) version: u64,
    pub(super) force_dirty: bool,
}

pub(super) fn pending_file_reload_matches_key(
    pending: &PendingFileReload,
    completed: &FileReloadCompletionKey<'_>,
) -> bool {
    pending.request_id == completed.request_id
        && pending.version == completed.version
        && pending.force_dirty == completed.force_dirty
        && file_paths_match_lexically(&pending.path, completed.path)
}

pub(super) fn next_file_reload_request_id(current: u64) -> u64 {
    match current.wrapping_add(1) {
        0 => 1,
        request_id => request_id,
    }
}

pub(super) fn pending_reload_is_clean_external_change(
    reload: &PendingFileReload,
    path: &Path,
) -> bool {
    !reload.force_dirty && file_paths_match_lexically(&reload.path, path)
}

pub(super) fn queued_reload_is_clean_external_change(
    reload: &QueuedFileReload,
    path: &Path,
) -> bool {
    !reload.force_dirty && file_paths_match_lexically(&reload.path, path)
}

pub(super) fn canceled_file_reload_key(
    id: BufferId,
    completed: &FileReloadCompletionKey<'_>,
) -> (BufferId, PendingFileReload) {
    (
        id,
        PendingFileReload {
            request_id: completed.request_id,
            path: completed.path.to_path_buf(),
            version: completed.version,
            force_dirty: completed.force_dirty,
        },
    )
}
