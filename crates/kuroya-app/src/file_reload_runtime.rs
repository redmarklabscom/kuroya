use crate::{
    KuroyaApp,
    app_state::{PendingFileReload, QueuedFileReload},
    devtools_async_tasks::file_reload_task_detail,
    file_io::read_text_file,
    file_runtime::loaded_text_buffer,
    image_preview::{image_preview_buffer_text, load_image_preview, path_is_image_preview},
    ui_events::UiEvent,
};
use kuroya_core::BufferId;
use std::{
    collections::{HashSet, hash_map::Entry},
    path::{Path, PathBuf},
    time::Instant,
};

mod keys;

pub(crate) use keys::file_paths_match_lexically;
use keys::{
    FileReloadCompletionKey, canceled_file_reload_key, next_file_reload_request_id,
    pending_file_reload_matches_key, pending_reload_is_clean_external_change,
    queued_reload_is_clean_external_change,
};

const CANCELED_FILE_RELOAD_CAP: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileReloadCompletion {
    Current,
    Stale,
    Untracked,
}

impl KuroyaApp {
    pub(crate) fn mark_buffer_changed_on_disk(&mut self, id: BufferId) -> bool {
        let changed = self.external_change_buffers.insert(id);
        if changed {
            self.bump_external_change_generation();
        }
        changed
    }

    pub(crate) fn clear_buffer_changed_on_disk(&mut self, id: BufferId) -> bool {
        let changed = self.external_change_buffers.remove(&id);
        if changed {
            self.bump_external_change_generation();
        }
        changed
    }

    pub(crate) fn clear_changed_on_disk_buffers(&mut self) -> bool {
        if self.external_change_buffers.is_empty() {
            return false;
        }
        self.external_change_buffers.clear();
        self.bump_external_change_generation();
        true
    }

    #[cfg(test)]
    pub(crate) fn buffer_changed_on_disk(&self, id: BufferId) -> bool {
        self.external_change_buffers.contains(&id)
    }

    #[cfg(test)]
    pub(crate) fn changed_on_disk_buffer_count(&self) -> usize {
        self.external_change_buffers.len()
    }

    pub(crate) fn buffer_has_observed_external_change(&self, id: BufferId) -> bool {
        if self.external_change_buffers.contains(&id) {
            return true;
        }

        let Some(buffer) = self.buffer(id) else {
            return false;
        };
        let Some(path) = buffer.path() else {
            return false;
        };
        self.buffer_has_pending_reload_external_change(id, path)
    }

    #[cfg(test)]
    pub(crate) fn observed_external_change_buffer_count(&self) -> usize {
        if !self.has_pending_reload_external_change_sources() {
            return self.external_change_buffers.len();
        }

        let mut count = self.external_change_buffers.len();
        for buffer in &self.buffers {
            let id = buffer.id();
            if self.external_change_buffers.contains(&id) {
                continue;
            }
            let Some(path) = buffer.path() else {
                continue;
            };
            if self.buffer_has_pending_reload_external_change(id, path) {
                count += 1;
            }
        }
        count
    }

    fn bump_external_change_generation(&mut self) {
        self.external_change_generation = self.external_change_generation.wrapping_add(1);
    }

    pub(crate) fn spawn_reload_clean_buffer(&mut self, id: BufferId, path: PathBuf) {
        let Some(buffer) = self.buffer(id) else {
            return;
        };
        let Some(buffer_path) = buffer.path() else {
            return;
        };
        if !file_paths_match_lexically(buffer_path, &path) {
            return;
        }
        if buffer.is_dirty() {
            self.mark_buffer_changed_on_disk(id);
            return;
        }
        let path = buffer_path.to_path_buf();
        let reload = if self.in_flight_reloads.contains_key(&id) {
            None
        } else {
            Some((buffer.version(), buffer.word_separators().to_owned()))
        };

        match reload {
            Some((version, word_separators)) => {
                self.spawn_reload_request(id, path, version, false, word_separators);
            }
            None => {
                self.queue_file_reload_request(id, path, false);
            }
        }
    }

    pub(crate) fn begin_reload_buffer_from_disk(&mut self, id: BufferId) {
        let Some(buffer) = self.buffer(id) else {
            return;
        };
        let has_path = buffer.path().is_some();
        let is_dirty = buffer.is_dirty();
        if !has_path {
            self.status = "Cannot reload an untitled buffer".to_owned();
            return;
        }
        self.set_active_buffer(id);
        if is_dirty {
            self.dirty_reload_buffer = Some(id);
            self.status = format!("Reload {} from disk?", self.file_io_buffer_label(id));
            return;
        }

        self.spawn_reload_buffer_from_disk(id, false);
    }

    pub(crate) fn spawn_reload_buffer_from_disk(&mut self, id: BufferId, force_dirty: bool) {
        enum DiskReloadRequest {
            Untitled,
            ConfirmDirty,
            Queue {
                path: PathBuf,
            },
            Spawn {
                path: PathBuf,
                version: u64,
                word_separators: String,
            },
        }

        let request = {
            let Some(buffer) = self.buffer(id) else {
                return;
            };
            match buffer.path() {
                None => DiskReloadRequest::Untitled,
                Some(_) if buffer.is_dirty() && !force_dirty => DiskReloadRequest::ConfirmDirty,
                Some(path) if self.in_flight_reloads.contains_key(&id) => {
                    DiskReloadRequest::Queue {
                        path: path.to_path_buf(),
                    }
                }
                Some(path) => DiskReloadRequest::Spawn {
                    path: path.to_path_buf(),
                    version: buffer.version(),
                    word_separators: buffer.word_separators().to_owned(),
                },
            }
        };

        match request {
            DiskReloadRequest::Untitled => {
                self.status = "Cannot reload an untitled buffer".to_owned();
            }
            DiskReloadRequest::ConfirmDirty => {
                self.dirty_reload_buffer = Some(id);
                self.status = format!("Reload {} from disk?", self.file_io_buffer_label(id));
            }
            DiskReloadRequest::Queue { path } => {
                self.queue_file_reload_request(id, path, force_dirty);
                if force_dirty {
                    self.status = format!("Queued reload {}", self.file_io_buffer_label(id));
                }
            }
            DiskReloadRequest::Spawn {
                path,
                version,
                word_separators,
            } => {
                self.spawn_reload_request(id, path, version, force_dirty, word_separators);
            }
        }
    }

    pub(crate) fn discard_and_reload_buffer_from_disk(&mut self, id: BufferId) {
        if self.save_conflict_buffer == Some(id) {
            self.save_conflict_buffer = None;
        }
        if self.dirty_reload_buffer == Some(id) {
            self.dirty_reload_buffer = None;
        }
        if self.close_after_save == Some(id) {
            self.close_after_save = None;
            self.pending_close_buffers.clear();
        }
        self.clear_deferred_save_work(id);
        self.cancel_deferred_reload_work(id);
        self.spawn_reload_buffer_from_disk(id, true);
    }

    fn spawn_reload_request(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        force_dirty: bool,
        word_separators: String,
    ) {
        let Some(request_id) =
            self.reserve_file_reload_request(id, path.clone(), version, force_dirty)
        else {
            return;
        };

        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        let tx = self.tx.clone();
        self.record_async_task_started("File Reload", file_reload_task_detail(request_id, &path));
        self.runtime.spawn(async move {
            let started = Instant::now();
            if path_is_image_preview(&path) {
                match load_image_preview(&path).await {
                    Ok(preview) => {
                        let buffer = loaded_text_buffer(
                            id,
                            path.clone(),
                            image_preview_buffer_text(&preview),
                            word_separators,
                        );
                        let _ = crate::ui_event_channel::send_critical_ui_event(
                            &tx,
                            UiEvent::ImageFileReloaded {
                                root,
                                generation,
                                request_id,
                                id,
                                path,
                                buffer,
                                preview,
                                elapsed: started.elapsed(),
                                version,
                                force_dirty,
                            },
                        );
                    }
                    Err(error) => {
                        let _ = crate::ui_event_channel::send_critical_ui_event(
                            &tx,
                            UiEvent::FileReloadFailed {
                                root,
                                generation,
                                request_id,
                                id,
                                path,
                                error,
                                version,
                                force_dirty,
                            },
                        );
                    }
                }
                return;
            }
            match read_text_file(&path).await {
                Ok(decoded) => {
                    let buffer =
                        loaded_text_buffer(id, path.clone(), decoded.text, word_separators);
                    let _ = crate::ui_event_channel::send_critical_ui_event(
                        &tx,
                        UiEvent::FileReloaded {
                            root,
                            generation,
                            request_id,
                            id,
                            path,
                            buffer,
                            elapsed: started.elapsed(),
                            version,
                            force_dirty,
                            lossy: decoded.lossy,
                            binary: decoded.binary,
                        },
                    );
                }
                Err(error) => {
                    let _ = crate::ui_event_channel::send_critical_ui_event(
                        &tx,
                        UiEvent::FileReloadFailed {
                            root,
                            generation,
                            request_id,
                            id,
                            path,
                            error,
                            version,
                            force_dirty,
                        },
                    );
                }
            }
        });
    }

    fn reserve_file_reload_request(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        force_dirty: bool,
    ) -> Option<u64> {
        if self.in_flight_reloads.contains_key(&id) {
            self.queue_file_reload_request(id, path, force_dirty);
            if force_dirty {
                self.status = format!("Queued reload {}", self.file_io_buffer_label(id));
            }
            return None;
        }

        let request_id = self.next_file_reload_request_id();
        self.in_flight_reloads.insert(
            id,
            PendingFileReload {
                request_id,
                path,
                version,
                force_dirty,
            },
        );
        Some(request_id)
    }

    pub(crate) fn finish_file_reload_request(
        &mut self,
        request_id: u64,
        id: BufferId,
        path: &Path,
        version: u64,
        force_dirty: bool,
    ) -> FileReloadCompletion {
        let completed = FileReloadCompletionKey {
            request_id,
            path,
            version,
            force_dirty,
        };
        let live_completion = match self.in_flight_reloads.entry(id) {
            Entry::Occupied(entry) if pending_file_reload_matches_key(entry.get(), &completed) => {
                entry.remove();
                FileReloadCompletion::Current
            }
            Entry::Occupied(_) => FileReloadCompletion::Stale,
            Entry::Vacant(_) => FileReloadCompletion::Untracked,
        };

        if live_completion == FileReloadCompletion::Current {
            let _ = self.take_matching_canceled_file_reload(id, &completed);
            return FileReloadCompletion::Current;
        }

        if self.take_matching_canceled_file_reload(id, &completed) {
            FileReloadCompletion::Stale
        } else {
            live_completion
        }
    }

    fn next_file_reload_request_id(&mut self) -> u64 {
        self.file_reload_next_request_id =
            next_file_reload_request_id(self.file_reload_next_request_id);
        self.file_reload_next_request_id
    }

    pub(crate) fn spawn_queued_reload_after_completion(&mut self, id: BufferId) {
        let Some(queued) = self.queued_file_reloads.remove(&id) else {
            return;
        };
        let Some(buffer) = self.buffer(id) else {
            return;
        };
        if !buffer
            .path()
            .is_some_and(|buffer_path| file_paths_match_lexically(buffer_path, &queued.path))
        {
            return;
        }
        if buffer.is_dirty() && !queued.force_dirty {
            self.mark_buffer_changed_on_disk(id);
            return;
        }
        let version = buffer.version();
        let word_separators = buffer.word_separators().to_owned();
        self.spawn_reload_request(
            id,
            queued.path,
            version,
            queued.force_dirty,
            word_separators,
        );
    }

    pub(crate) fn cancel_deferred_reload_work(&mut self, id: BufferId) {
        if let Some(pending) = self.in_flight_reloads.remove(&id) {
            self.cancel_file_reload_request(id, pending);
        }
        self.queued_file_reloads.remove(&id);
    }

    fn cancel_file_reload_request(&mut self, id: BufferId, pending: PendingFileReload) {
        let canceled = (id, pending);
        if self.canceled_file_reloads.insert(canceled.clone()) {
            self.canceled_file_reload_order.push_back(canceled);
        }
        while self.canceled_file_reloads.len() > CANCELED_FILE_RELOAD_CAP {
            let Some(oldest) = self.canceled_file_reload_order.pop_front() else {
                break;
            };
            self.canceled_file_reloads.remove(&oldest);
        }
    }

    fn take_canceled_file_reload(&mut self, canceled: &(BufferId, PendingFileReload)) -> bool {
        let removed = self.canceled_file_reloads.remove(canceled);
        if removed {
            if let Some(index) = self
                .canceled_file_reload_order
                .iter()
                .position(|queued| queued == canceled)
            {
                self.canceled_file_reload_order.remove(index);
            }
        }
        removed
    }

    fn take_matching_canceled_file_reload(
        &mut self,
        id: BufferId,
        completed: &FileReloadCompletionKey<'_>,
    ) -> bool {
        let exact = canceled_file_reload_key(id, completed);
        if self.take_canceled_file_reload(&exact) {
            return true;
        }

        let canceled = self
            .canceled_file_reloads
            .iter()
            .find(|(canceled_id, canceled)| {
                *canceled_id == id && pending_file_reload_matches_key(canceled, completed)
            })
            .cloned();
        canceled.is_some_and(|canceled| self.take_canceled_file_reload(&canceled))
    }

    fn queue_file_reload_request(&mut self, id: BufferId, path: PathBuf, force_dirty: bool) {
        match self.queued_file_reloads.entry(id) {
            Entry::Occupied(mut entry) => {
                let queued = entry.get_mut();
                if force_dirty || !queued.force_dirty {
                    queued.path = path;
                }
                queued.force_dirty |= force_dirty;
            }
            Entry::Vacant(entry) => {
                entry.insert(QueuedFileReload { path, force_dirty });
            }
        }
    }

    pub(crate) fn mark_unapplied_file_reload_as_external_change(
        &mut self,
        id: BufferId,
        path: &Path,
        force_dirty: bool,
    ) {
        if force_dirty {
            return;
        }
        if self.buffer(id).is_some_and(|buffer| {
            buffer
                .path()
                .is_some_and(|buffer_path| file_paths_match_lexically(buffer_path, path))
        }) {
            self.mark_buffer_changed_on_disk(id);
        }
    }

    pub(crate) fn save_needs_observed_external_change_confirmation(
        &self,
        id: BufferId,
        path: &Path,
    ) -> bool {
        let Some(buffer) = self.buffer(id) else {
            return false;
        };
        if !buffer
            .path()
            .is_some_and(|buffer_path| file_paths_match_lexically(buffer_path, path))
        {
            return false;
        }

        self.external_change_buffers.contains(&id)
            || self.buffer_has_pending_reload_external_change(id, path)
    }

    pub(crate) fn observed_external_change_buffer_ids(&self) -> HashSet<BufferId> {
        if !self.has_pending_reload_external_change_sources() {
            return self.external_change_buffers.clone();
        }

        let mut ids = HashSet::with_capacity(
            self.external_change_buffers
                .len()
                .saturating_add(self.pending_reload_external_change_capacity_hint()),
        );
        ids.extend(self.external_change_buffers.iter().copied());
        self.extend_pending_reload_external_change_buffer_ids(&mut ids);
        ids
    }

    pub(crate) fn pending_reload_external_change_buffer_ids(&self) -> Vec<BufferId> {
        if !self.has_pending_reload_external_change_sources() {
            return Vec::new();
        }

        let mut ids = Vec::with_capacity(self.pending_reload_external_change_capacity_hint());
        self.extend_pending_reload_external_change_buffer_ids(&mut ids);
        ids
    }

    pub(crate) fn has_pending_reload_external_change_sources(&self) -> bool {
        !(self.in_flight_reloads.is_empty() && self.queued_file_reloads.is_empty())
    }

    fn pending_reload_external_change_capacity_hint(&self) -> usize {
        self.buffers.len().min(
            self.in_flight_reloads
                .len()
                .saturating_add(self.queued_file_reloads.len()),
        )
    }

    fn extend_pending_reload_external_change_buffer_ids(&self, ids: &mut impl Extend<BufferId>) {
        ids.extend(self.buffers.iter().filter_map(|buffer| {
            let id = buffer.id();
            let path = buffer.path()?;
            self.buffer_has_pending_reload_external_change(id, path)
                .then_some(id)
        }));
    }

    fn buffer_has_pending_reload_external_change(&self, id: BufferId, path: &Path) -> bool {
        self.in_flight_reloads
            .get(&id)
            .is_some_and(|reload| pending_reload_is_clean_external_change(reload, path))
            || self
                .queued_file_reloads
                .get(&id)
                .is_some_and(|reload| queued_reload_is_clean_external_change(reload, path))
    }

    pub(crate) fn open_save_conflict_for_buffer(&mut self, id: BufferId) {
        self.save_conflict_buffer.get_or_insert(id);
        self.set_active_buffer(id);
        self.status = format!("{} changed on disk", self.file_io_buffer_label(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn changed_on_disk_generation_tracks_real_marker_changes_only() {
        let mut app = app_for_test(PathBuf::from("workspace"));

        assert_eq!(app.external_change_generation, 0);
        assert!(app.mark_buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, 1);

        assert!(!app.mark_buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, 1);

        assert!(app.clear_buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, 2);

        assert!(!app.clear_buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, 2);

        assert!(app.mark_buffer_changed_on_disk(7));
        assert!(app.mark_buffer_changed_on_disk(8));
        assert_eq!(app.external_change_generation, 4);

        assert!(app.clear_changed_on_disk_buffers());
        assert_eq!(app.external_change_generation, 5);

        assert!(!app.clear_changed_on_disk_buffers());
        assert_eq!(app.external_change_generation, 5);
    }

    #[test]
    fn next_file_reload_request_id_wraps_without_zero() {
        assert_eq!(next_file_reload_request_id(0), 1);
        assert_eq!(next_file_reload_request_id(7), 8);
        assert_eq!(next_file_reload_request_id(u64::MAX - 1), u64::MAX);
        assert_eq!(next_file_reload_request_id(u64::MAX), 1);
    }

    #[test]
    fn clean_external_change_helpers_ignore_force_dirty_reloads() {
        let path = PathBuf::from("workspace/src/main.rs");
        let equivalent_path = PathBuf::from("workspace/src/./main.rs");
        let pending = PendingFileReload {
            request_id: 7,
            path: equivalent_path.clone(),
            version: 3,
            force_dirty: false,
        };
        let force_dirty_pending = PendingFileReload {
            force_dirty: true,
            ..pending.clone()
        };
        let queued = QueuedFileReload {
            path: equivalent_path.clone(),
            force_dirty: false,
        };
        let force_dirty_queued = QueuedFileReload {
            path: equivalent_path,
            force_dirty: true,
        };

        assert!(pending_reload_is_clean_external_change(&pending, &path));
        assert!(!pending_reload_is_clean_external_change(
            &force_dirty_pending,
            &path
        ));
        assert!(queued_reload_is_clean_external_change(&queued, &path));
        assert!(!queued_reload_is_clean_external_change(
            &force_dirty_queued,
            &path
        ));
    }

    #[test]
    fn canceled_file_reload_tombstones_are_bounded_in_insertion_order() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());

        for index in 0..(CANCELED_FILE_RELOAD_CAP + 3) {
            app.cancel_file_reload_request(
                index as BufferId,
                PendingFileReload {
                    request_id: index as u64 + 1,
                    path: root.join(format!("src/file_{index}.rs")),
                    version: index as u64,
                    force_dirty: false,
                },
            );
        }

        assert_eq!(app.canceled_file_reloads.len(), CANCELED_FILE_RELOAD_CAP);
        assert_eq!(
            app.canceled_file_reload_order.len(),
            CANCELED_FILE_RELOAD_CAP
        );
        assert!(!app.canceled_file_reloads.contains(&(
            0,
            PendingFileReload {
                request_id: 1,
                path: root.join("src/file_0.rs"),
                version: 0,
                force_dirty: false,
            },
        )));
        assert!(app.canceled_file_reloads.contains(&(
            (CANCELED_FILE_RELOAD_CAP + 2) as BufferId,
            PendingFileReload {
                request_id: CANCELED_FILE_RELOAD_CAP as u64 + 3,
                path: root.join(format!("src/file_{}.rs", CANCELED_FILE_RELOAD_CAP + 2)),
                version: CANCELED_FILE_RELOAD_CAP as u64 + 2,
                force_dirty: false,
            },
        )));
        assert_eq!(
            app.canceled_file_reload_order
                .front()
                .map(|(_, pending)| pending.request_id),
            Some(4)
        );
    }

    #[test]
    fn finish_canceled_file_reload_request_removes_tombstone_order_entry() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let pending = PendingFileReload {
            request_id: 9,
            path: path.clone(),
            version: 3,
            force_dirty: false,
        };
        app.cancel_file_reload_request(7, pending.clone());

        assert_eq!(
            app.finish_file_reload_request(
                pending.request_id,
                7,
                &path,
                pending.version,
                pending.force_dirty,
            ),
            FileReloadCompletion::Stale
        );

        assert!(app.canceled_file_reloads.is_empty());
        assert!(app.canceled_file_reload_order.is_empty());
    }

    #[test]
    fn finish_canceled_file_reload_request_matches_equivalent_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join("..").join("src").join("main.rs");
        let mut app = app_for_test(root);
        let pending = PendingFileReload {
            request_id: 9,
            path,
            version: 3,
            force_dirty: false,
        };
        app.cancel_file_reload_request(7, pending.clone());

        assert_eq!(
            app.finish_file_reload_request(
                pending.request_id,
                7,
                &equivalent_path,
                pending.version,
                pending.force_dirty,
            ),
            FileReloadCompletion::Stale
        );

        assert!(app.canceled_file_reloads.is_empty());
        assert!(app.canceled_file_reload_order.is_empty());
    }

    #[test]
    fn finish_canceled_file_reload_request_prefers_exact_tombstone_over_equivalent_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join("..").join("src").join("main.rs");
        let mut app = app_for_test(root);
        let exact = PendingFileReload {
            request_id: 9,
            path: path.clone(),
            version: 3,
            force_dirty: false,
        };
        let equivalent = PendingFileReload {
            path: equivalent_path,
            ..exact.clone()
        };
        app.cancel_file_reload_request(7, equivalent.clone());
        app.cancel_file_reload_request(7, exact.clone());

        assert_eq!(
            app.finish_file_reload_request(
                exact.request_id,
                7,
                &path,
                exact.version,
                exact.force_dirty,
            ),
            FileReloadCompletion::Stale
        );

        assert!(!app.canceled_file_reloads.contains(&(7, exact)));
        assert!(app.canceled_file_reloads.contains(&(7, equivalent.clone())));
        assert_eq!(
            app.canceled_file_reload_order,
            std::collections::VecDeque::from([(7, equivalent)])
        );
    }

    #[test]
    fn finish_canceled_file_reload_request_rejects_key_mismatch_without_consuming_tombstone() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let cases = [
            ("path", root.join("src/other.rs"), 3, false),
            ("version", path.clone(), 4, false),
            ("force_dirty", path.clone(), 3, true),
        ];

        for (case, completed_path, completed_version, completed_force_dirty) in cases {
            let mut app = app_for_test(root.clone());
            let pending = PendingFileReload {
                request_id: 9,
                path: path.clone(),
                version: 3,
                force_dirty: false,
            };
            app.cancel_file_reload_request(7, pending.clone());

            assert_eq!(
                app.finish_file_reload_request(
                    pending.request_id,
                    7,
                    &completed_path,
                    completed_version,
                    completed_force_dirty,
                ),
                FileReloadCompletion::Untracked,
                "{case}"
            );

            assert!(
                app.canceled_file_reloads.contains(&(7, pending.clone())),
                "{case}"
            );
            assert_eq!(
                app.canceled_file_reload_order,
                std::collections::VecDeque::from([(7, pending)]),
                "{case}"
            );
        }
    }

    #[test]
    fn duplicate_canceled_file_reload_tombstone_is_not_enqueued_twice() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let pending = PendingFileReload {
            request_id: 9,
            path,
            version: 3,
            force_dirty: false,
        };

        app.cancel_file_reload_request(7, pending.clone());
        app.cancel_file_reload_request(7, pending.clone());

        assert_eq!(app.canceled_file_reloads.len(), 1);
        assert_eq!(app.canceled_file_reload_order.len(), 1);
        assert!(app.canceled_file_reloads.contains(&(7, pending.clone())));
        assert_eq!(
            app.canceled_file_reload_order,
            std::collections::VecDeque::from([(7, pending)])
        );
    }

    #[test]
    fn finish_current_file_reload_request_removes_in_flight_and_preserves_unrelated_tombstones() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let tombstone_path = root.join("src/old.rs");
        let mut app = app_for_test(root);
        let pending = PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version: 5,
            force_dirty: false,
        };
        let tombstone = PendingFileReload {
            request_id: 9,
            path: tombstone_path,
            version: 3,
            force_dirty: false,
        };
        app.in_flight_reloads.insert(7, pending.clone());
        app.cancel_file_reload_request(8, tombstone.clone());

        assert_eq!(
            app.finish_file_reload_request(
                pending.request_id,
                7,
                &path,
                pending.version,
                pending.force_dirty,
            ),
            FileReloadCompletion::Current
        );

        assert!(!app.in_flight_reloads.contains_key(&7));
        assert!(app.canceled_file_reloads.contains(&(8, tombstone.clone())));
        assert_eq!(
            app.canceled_file_reload_order,
            std::collections::VecDeque::from([(8, tombstone)])
        );
    }

    #[test]
    fn finish_current_file_reload_request_wins_over_matching_stale_tombstone() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let pending = PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version: 5,
            force_dirty: false,
        };
        app.in_flight_reloads.insert(7, pending.clone());
        app.cancel_file_reload_request(7, pending.clone());

        assert_eq!(
            app.finish_file_reload_request(
                pending.request_id,
                7,
                &path,
                pending.version,
                pending.force_dirty,
            ),
            FileReloadCompletion::Current
        );

        assert!(!app.in_flight_reloads.contains_key(&7));
        assert!(app.canceled_file_reloads.is_empty());
        assert!(app.canceled_file_reload_order.is_empty());
    }

    #[test]
    fn finish_stale_file_reload_request_rejects_in_flight_key_mismatch() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let cases = [
            ("path", root.join("src/other.rs"), 5, false),
            ("version", path.clone(), 6, false),
            ("force_dirty", path.clone(), 5, true),
        ];

        for (case, completed_path, completed_version, completed_force_dirty) in cases {
            let mut app = app_for_test(root.clone());
            let current = PendingFileReload {
                request_id: 11,
                path: path.clone(),
                version: 5,
                force_dirty: false,
            };
            app.in_flight_reloads.insert(7, current.clone());

            assert_eq!(
                app.finish_file_reload_request(
                    current.request_id,
                    7,
                    &completed_path,
                    completed_version,
                    completed_force_dirty,
                ),
                FileReloadCompletion::Stale,
                "{case}"
            );

            assert_eq!(app.in_flight_reloads.get(&7), Some(&current), "{case}");
        }
    }

    #[test]
    fn finish_stale_file_reload_request_preserves_newer_in_flight_and_tombstone_order() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let tombstone_path = root.join("src/old.rs");
        let mut app = app_for_test(root);
        let current = PendingFileReload {
            request_id: 11,
            path: path.clone(),
            version: 5,
            force_dirty: false,
        };
        let tombstone = PendingFileReload {
            request_id: 9,
            path: tombstone_path,
            version: 3,
            force_dirty: false,
        };
        app.in_flight_reloads.insert(7, current.clone());
        app.cancel_file_reload_request(8, tombstone.clone());

        assert_eq!(
            app.finish_file_reload_request(10, 7, &path, current.version, current.force_dirty),
            FileReloadCompletion::Stale
        );

        assert_eq!(app.in_flight_reloads.get(&7), Some(&current));
        assert!(app.canceled_file_reloads.contains(&(8, tombstone.clone())));
        assert_eq!(
            app.canceled_file_reload_order,
            std::collections::VecDeque::from([(8, tombstone)])
        );
    }

    #[test]
    fn finish_untracked_file_reload_request_preserves_tombstone_order() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let tombstone_path = root.join("src/old.rs");
        let mut app = app_for_test(root);
        let tombstone = PendingFileReload {
            request_id: 9,
            path: tombstone_path,
            version: 3,
            force_dirty: false,
        };
        app.cancel_file_reload_request(8, tombstone.clone());

        assert_eq!(
            app.finish_file_reload_request(10, 7, &path, 5, false),
            FileReloadCompletion::Untracked
        );

        assert!(app.canceled_file_reloads.contains(&(8, tombstone.clone())));
        assert_eq!(
            app.canceled_file_reload_order,
            std::collections::VecDeque::from([(8, tombstone)])
        );
    }

    #[test]
    fn observed_external_change_buffer_ids_merges_markers_and_pending_clean_reloads() {
        let root = PathBuf::from("workspace");
        let first = root.join("src/first.rs");
        let second = root.join("src/second.rs");
        let third = root.join("src/third.rs");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(1, Some(first), "first".to_owned()));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(second.clone()),
            "second".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            3,
            Some(third.clone()),
            "third".to_owned(),
        ));
        app.mark_buffer_changed_on_disk(1);
        app.in_flight_reloads.insert(
            2,
            PendingFileReload {
                request_id: 1,
                path: second,
                version: app.buffer(2).expect("buffer should exist").version(),
                force_dirty: false,
            },
        );
        app.queued_file_reloads.insert(
            3,
            QueuedFileReload {
                path: third,
                force_dirty: false,
            },
        );

        let ids = app.observed_external_change_buffer_ids();

        assert_eq!(ids, HashSet::from([1, 2, 3]));
    }

    #[test]
    fn observed_external_change_buffer_ids_ignores_queued_force_dirty_and_path_mismatches() {
        let root = PathBuf::from("workspace");
        let first = root.join("src/first.rs");
        let second = root.join("src/second.rs");
        let other = root.join("src/other.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(first.clone()),
            "first".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(second.clone()),
            "second".to_owned(),
        ));
        app.queued_file_reloads.insert(
            1,
            QueuedFileReload {
                path: first,
                force_dirty: true,
            },
        );
        app.queued_file_reloads.insert(
            2,
            QueuedFileReload {
                path: other,
                force_dirty: false,
            },
        );

        let ids = app.observed_external_change_buffer_ids();

        assert!(ids.is_empty());
    }

    #[test]
    fn buffer_has_observed_external_change_tracks_markers_and_pending_clean_reloads() {
        let root = PathBuf::from("workspace");
        let first = root.join("src/first.rs");
        let second = root.join("src/second.rs");
        let third = root.join("src/third.rs");
        let fourth = root.join("src/fourth.rs");
        let fifth = root.join("src/fifth.rs");
        let other = root.join("src/other.rs");
        let mut app = app_for_test(root);
        for (id, path) in [
            (1, first.clone()),
            (2, second.clone()),
            (3, third.clone()),
            (4, fourth.clone()),
            (5, fifth.clone()),
        ] {
            app.buffers
                .push(TextBuffer::from_text(id, Some(path), String::new()));
        }
        app.mark_buffer_changed_on_disk(1);
        app.in_flight_reloads.insert(
            2,
            PendingFileReload {
                request_id: 1,
                path: second,
                version: app.buffer(2).expect("buffer should exist").version(),
                force_dirty: false,
            },
        );
        app.queued_file_reloads.insert(
            3,
            QueuedFileReload {
                path: third,
                force_dirty: false,
            },
        );
        app.in_flight_reloads.insert(
            4,
            PendingFileReload {
                request_id: 2,
                path: fourth,
                version: app.buffer(4).expect("buffer should exist").version(),
                force_dirty: true,
            },
        );
        app.queued_file_reloads.insert(
            5,
            QueuedFileReload {
                path: other,
                force_dirty: false,
            },
        );

        assert!(app.buffer_has_observed_external_change(1));
        assert!(app.buffer_has_observed_external_change(2));
        assert!(app.buffer_has_observed_external_change(3));
        assert!(!app.buffer_has_observed_external_change(4));
        assert!(!app.buffer_has_observed_external_change(5));
        assert!(!app.buffer_has_observed_external_change(99));
    }

    #[test]
    fn observed_external_change_buffer_count_dedupes_markers_and_pending_clean_reloads() {
        let root = PathBuf::from("workspace");
        let first = root.join("src/first.rs");
        let second = root.join("src/second.rs");
        let third = root.join("src/third.rs");
        let fourth = root.join("src/fourth.rs");
        let mut app = app_for_test(root);
        for (id, path) in [
            (1, first.clone()),
            (2, second.clone()),
            (3, third.clone()),
            (4, fourth.clone()),
        ] {
            app.buffers
                .push(TextBuffer::from_text(id, Some(path), String::new()));
        }
        app.mark_buffer_changed_on_disk(1);
        app.mark_buffer_changed_on_disk(2);
        app.in_flight_reloads.insert(
            2,
            PendingFileReload {
                request_id: 1,
                path: second,
                version: app.buffer(2).expect("buffer should exist").version(),
                force_dirty: false,
            },
        );
        app.queued_file_reloads.insert(
            3,
            QueuedFileReload {
                path: third,
                force_dirty: false,
            },
        );
        app.queued_file_reloads.insert(
            4,
            QueuedFileReload {
                path: fourth,
                force_dirty: true,
            },
        );

        assert_eq!(app.changed_on_disk_buffer_count(), 2);
        assert_eq!(app.observed_external_change_buffer_count(), 3);
    }

    #[test]
    fn save_confirmation_ignores_force_dirty_reload_and_path_mismatches() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other = root.join("src/other.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "main".to_owned(),
        ));
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path: path.clone(),
                version: app.buffer(7).expect("buffer should exist").version(),
                force_dirty: true,
            },
        );

        assert!(!app.save_needs_observed_external_change_confirmation(7, &path));

        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 2,
                path: other.clone(),
                version: app.buffer(7).expect("buffer should exist").version(),
                force_dirty: false,
            },
        );

        assert!(!app.save_needs_observed_external_change_confirmation(7, &path));
        assert!(!app.save_needs_observed_external_change_confirmation(7, &other));
    }

    #[test]
    fn save_confirmation_tracks_queued_clean_reload_guards() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other = root.join("src/other.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "main".to_owned(),
        ));
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path: path.clone(),
                force_dirty: false,
            },
        );

        assert!(app.save_needs_observed_external_change_confirmation(7, &path));
        assert!(!app.save_needs_observed_external_change_confirmation(7, &other));

        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path: path.clone(),
                force_dirty: true,
            },
        );

        assert!(!app.save_needs_observed_external_change_confirmation(7, &path));

        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path: other,
                force_dirty: false,
            },
        );

        assert!(!app.save_needs_observed_external_change_confirmation(7, &path));
    }

    #[test]
    fn dirty_reload_prompt_does_not_queue_or_spawn_reload_work() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "main".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        let version = app.buffer(7).expect("buffer should exist").version();
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path,
                version,
                force_dirty: false,
            },
        );

        app.spawn_reload_buffer_from_disk(7, false);

        assert_eq!(app.dirty_reload_buffer, Some(7));
        assert!(app.queued_file_reloads.is_empty());
        assert_eq!(app.in_flight_reloads.len(), 1);
        assert!(app.active_async_tasks.is_empty());
        assert_eq!(app.status, "Reload main.rs from disk?");
    }

    #[test]
    fn queued_force_dirty_reload_is_not_downgraded_by_later_clean_reload() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "main".to_owned(),
        ));
        let version = app.buffer(7).expect("buffer should exist").version();
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path: path.clone(),
                version,
                force_dirty: false,
            },
        );

        app.spawn_reload_buffer_from_disk(7, true);
        app.spawn_reload_clean_buffer(7, other);

        assert_eq!(
            app.queued_file_reloads.get(&7),
            Some(&QueuedFileReload {
                path,
                force_dirty: true,
            })
        );
    }

    #[test]
    fn queued_clean_reload_preserves_buffer_raw_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "main".to_owned(),
        ));
        let version = app.buffer(7).expect("buffer should exist").version();
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path: path.clone(),
                version,
                force_dirty: false,
            },
        );

        app.spawn_reload_clean_buffer(7, path.clone());
        app.spawn_reload_clean_buffer(7, equivalent_path);

        assert_eq!(
            app.queued_file_reloads.get(&7),
            Some(&QueuedFileReload {
                path,
                force_dirty: false,
            })
        );
    }

    #[test]
    fn clean_reload_spawn_preserves_buffer_raw_path_for_equivalent_change() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "main".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);

        app.spawn_reload_clean_buffer(7, equivalent_path);

        assert_eq!(
            app.in_flight_reloads.get(&7),
            Some(&PendingFileReload {
                request_id: 1,
                path,
                version,
                force_dirty: false,
            })
        );
    }

    #[test]
    fn clean_reload_request_for_dirty_same_path_marks_changed_on_disk_without_work() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "main".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        let generation = app.external_change_generation;

        app.spawn_reload_clean_buffer(7, path);

        assert!(app.buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, generation + 1);
        assert!(app.in_flight_reloads.is_empty());
        assert!(app.queued_file_reloads.is_empty());
        assert!(app.active_async_tasks.is_empty());
    }

    #[test]
    fn clean_reload_request_for_dirty_equivalent_path_marks_changed_on_disk() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), "main".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);

        app.spawn_reload_clean_buffer(7, equivalent_path);

        assert!(app.buffer_changed_on_disk(7));
        assert!(app.in_flight_reloads.is_empty());
        assert!(app.queued_file_reloads.is_empty());
    }

    #[test]
    fn clean_reload_request_for_dirty_mismatched_path_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other = root.join("src/other.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), "main".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        let generation = app.external_change_generation;

        app.spawn_reload_clean_buffer(7, other);

        assert!(!app.buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, generation);
        assert!(app.in_flight_reloads.is_empty());
        assert!(app.queued_file_reloads.is_empty());
    }

    #[test]
    fn queued_force_dirty_reload_upgrades_existing_clean_reload() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "main".to_owned(),
        ));
        let version = app.buffer(7).expect("buffer should exist").version();
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path: path.clone(),
                version,
                force_dirty: false,
            },
        );

        app.spawn_reload_clean_buffer(7, path.clone());
        app.spawn_reload_buffer_from_disk(7, true);

        assert_eq!(
            app.queued_file_reloads.get(&7),
            Some(&QueuedFileReload {
                path: path.clone(),
                force_dirty: true,
            })
        );
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }
}
