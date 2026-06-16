use crate::{
    KuroyaApp,
    app_state::PendingFormatOnSave,
    file_history::{LOCAL_HISTORY_MAX_BYTES, snapshot_file_before_save_async},
    file_io::write_text_snapshot_atomic_async,
    large_file_mode::buffer_uses_large_file_mode,
    path_display::display_path_label_cow,
    save_lifecycle::{
        SaveRequest, finish_save_request, protected_preview_save_block_reason_for_buffer,
        reserve_save_request,
    },
    ui_events::UiEvent,
    workspace_state::paths_match_lexically,
};
use kuroya_core::{BufferId, TextBuffer};
use std::{path::PathBuf, time::Instant};

const CANCELED_FORMATTING_REQUEST_CAP: usize = 512;

impl KuroyaApp {
    pub(crate) fn clear_deferred_save_work(&mut self, id: BufferId) {
        self.queued_save_paths.remove(&id);
        self.cancel_pending_format_on_save(id);
        self.format_on_save_bypass.remove(&id);
    }

    pub(crate) fn begin_pending_format_on_save(
        &mut self,
        id: BufferId,
        pending: PendingFormatOnSave,
    ) {
        self.replace_pending_format_on_save(id, pending, 0);
    }

    pub(crate) fn queue_pending_format_on_save(
        &mut self,
        id: BufferId,
        pending: PendingFormatOnSave,
    ) {
        self.pending_format_on_save.insert(id, pending);
        self.pending_format_on_save_started
            .entry(id)
            .or_insert_with(Instant::now);
        self.pending_format_on_save_retries.entry(id).or_insert(0);
    }

    pub(crate) fn replace_pending_format_on_save(
        &mut self,
        id: BufferId,
        pending: PendingFormatOnSave,
        retries: u8,
    ) {
        let request_id = pending.request_id;
        if let Some(previous) = self.pending_format_on_save.insert(id, pending)
            && request_id != previous.request_id
        {
            self.cancel_lsp_formatting_request(previous.request_id);
        }
        self.pending_format_on_save_started
            .insert(id, Instant::now());
        self.pending_format_on_save_retries.insert(id, retries);
    }

    pub(crate) fn finish_pending_format_on_save(
        &mut self,
        id: BufferId,
    ) -> Option<PendingFormatOnSave> {
        self.pending_format_on_save_started.remove(&id);
        self.pending_format_on_save_retries.remove(&id);
        self.pending_format_on_save.remove(&id)
    }

    pub(crate) fn cancel_pending_format_on_save(&mut self, id: BufferId) {
        if let Some(pending) = self.pending_format_on_save.remove(&id) {
            self.cancel_lsp_formatting_request(pending.request_id);
        }
        self.pending_format_on_save_started.remove(&id);
        self.pending_format_on_save_retries.remove(&id);
        self.clear_format_on_save_overwrite_external_change(id);
    }

    pub(crate) fn mark_format_on_save_overwrite_external_change(&mut self, id: BufferId) -> bool {
        self.format_on_save_overwrite_external_change.insert(id)
    }

    pub(crate) fn clear_format_on_save_overwrite_external_change(&mut self, id: BufferId) -> bool {
        self.format_on_save_overwrite_external_change.remove(&id)
    }

    pub(crate) fn take_format_on_save_overwrite_external_change(&mut self, id: BufferId) -> bool {
        self.clear_format_on_save_overwrite_external_change(id)
    }

    pub(crate) fn format_on_save_overwrites_external_change(&self, id: BufferId) -> bool {
        self.format_on_save_overwrite_external_change.contains(&id)
    }

    pub(crate) fn clear_format_on_save_overwrite_external_changes(&mut self) {
        self.format_on_save_overwrite_external_change.clear();
    }

    pub(crate) fn pending_format_on_save_retries(&self, id: BufferId) -> u8 {
        self.pending_format_on_save_retries
            .get(&id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn cancel_lsp_formatting_request(&mut self, request_id: u64) {
        if self.canceled_formatting_request_ids.insert(request_id) {
            self.canceled_formatting_request_order.push_back(request_id);
        }
        while self.canceled_formatting_request_ids.len() > CANCELED_FORMATTING_REQUEST_CAP {
            let Some(oldest) = self.canceled_formatting_request_order.pop_front() else {
                break;
            };
            self.canceled_formatting_request_ids.remove(&oldest);
        }
    }

    pub(crate) fn take_canceled_lsp_formatting_request(&mut self, request_id: u64) -> bool {
        let removed = self.canceled_formatting_request_ids.remove(&request_id);
        if removed {
            if let Some(index) = self
                .canceled_formatting_request_order
                .iter()
                .position(|queued| *queued == request_id)
            {
                self.canceled_formatting_request_order.remove(index);
            }
        }
        removed
    }

    pub(crate) fn spawn_save_to(&mut self, id: BufferId, path: PathBuf) {
        self.spawn_save_to_inner(id, path, true);
    }

    pub(crate) fn spawn_save_to_over_external_change(&mut self, id: BufferId, path: PathBuf) {
        self.spawn_save_to_inner(id, path, false);
    }

    fn spawn_save_to_inner(
        &mut self,
        id: BufferId,
        path: PathBuf,
        confirm_observed_external_change: bool,
    ) {
        if confirm_observed_external_change {
            self.clear_format_on_save_overwrite_external_change(id);
        }
        let protected_reason = {
            let Some(buffer) = self.buffer(id) else {
                return;
            };
            protected_preview_save_block_reason_for_buffer(
                buffer,
                &self.lossy_decoded_buffers,
                &self.binary_preview_buffers,
            )
        };
        if let Some(reason) = protected_reason {
            self.block_protected_preview_save(id, reason);
            return;
        }
        if confirm_observed_external_change
            && self.save_needs_observed_external_change_confirmation(id, &path)
        {
            self.open_save_conflict_for_buffer(id);
            return;
        }
        let pending_format = if self.format_on_save_bypass.contains(&id) {
            None
        } else {
            self.pending_format_on_save.get(&id).map(|pending| {
                let buffer = self
                    .buffer(id)
                    .expect("save target should exist while formatting before save");
                let current_format_path = buffer.path().cloned().unwrap_or_else(|| path.clone());
                let format_path_changed =
                    !paths_match_lexically(&current_format_path, &pending.format_path);
                (
                    pending.version,
                    pending.request_id,
                    buffer.version(),
                    current_format_path,
                    format_path_changed,
                )
            })
        };

        if let Some((
            pending_version,
            pending_request_id,
            current_version,
            current_format_path,
            format_path_changed,
        )) = pending_format
        {
            if !confirm_observed_external_change {
                self.mark_format_on_save_overwrite_external_change(id);
            }
            let next_pending = PendingFormatOnSave {
                save_path: path.clone(),
                format_path: current_format_path,
                version: current_version,
                request_id: pending_request_id,
            };
            if current_version != pending_version || format_path_changed {
                if let Some(request_id) = self.request_lsp_formatting_for_buffer(
                    id,
                    Some("Formatting before save"),
                    false,
                ) {
                    self.replace_pending_format_on_save(
                        id,
                        PendingFormatOnSave {
                            request_id,
                            ..next_pending
                        },
                        0,
                    );
                    return;
                }
                self.cancel_pending_format_on_save(id);
            } else {
                self.queue_pending_format_on_save(id, next_pending);
                self.status = format!("Queued save {}", display_path_label_cow(&path));
                return;
            }
        }
        self.spawn_save_after_pending_format_check(id, path, confirm_observed_external_change);
    }

    fn spawn_save_after_pending_format_check(
        &mut self,
        id: BufferId,
        path: PathBuf,
        confirm_observed_external_change: bool,
    ) {
        if self.in_flight_saves.contains(&id) {
            if reserve_save_request(
                id,
                &path,
                &mut self.in_flight_saves,
                &mut self.queued_save_paths,
            ) == SaveRequest::Queued
            {
                self.status = format!("Queued save {}", display_path_label_cow(&path));
            }
            return;
        }
        let skip_format_on_save = self.format_on_save_bypass.remove(&id);
        if self.settings.format_on_save && !skip_format_on_save {
            let (version, format_path) = self
                .buffer(id)
                .map(|buffer| {
                    (
                        buffer.version(),
                        buffer.path().cloned().unwrap_or_else(|| path.clone()),
                    )
                })
                .expect("save target should exist while requesting format-on-save");
            if let Some(request_id) =
                self.request_lsp_formatting_for_buffer(id, Some("Formatting before save"), false)
            {
                if !confirm_observed_external_change {
                    self.mark_format_on_save_overwrite_external_change(id);
                }
                self.begin_pending_format_on_save(
                    id,
                    PendingFormatOnSave {
                        save_path: path.clone(),
                        format_path,
                        version,
                        request_id,
                    },
                );
                return;
            }
        }
        if !confirm_observed_external_change {
            self.clear_format_on_save_overwrite_external_change(id);
        }
        if reserve_save_request(
            id,
            &path,
            &mut self.in_flight_saves,
            &mut self.queued_save_paths,
        ) == SaveRequest::Queued
        {
            self.status = format!("Queued save {}", display_path_label_cow(&path));
            return;
        }
        let cleanup_changed = {
            let trim_trailing_whitespace = self.settings.trim_trailing_whitespace;
            let insert_final_newline = self.settings.insert_final_newline;
            let trim_final_newlines = self.settings.trim_final_newlines;
            self.buffer_mut(id).is_some_and(|buffer| {
                apply_save_cleanup_for_buffer(
                    buffer,
                    trim_trailing_whitespace,
                    insert_final_newline,
                    trim_final_newlines,
                )
            })
        };
        if cleanup_changed {
            self.mark_buffer_changed(id);
        }
        let Some(buffer) = self.buffer(id) else {
            finish_save_request(id, &mut self.in_flight_saves, &mut self.queued_save_paths);
            return;
        };
        let text = buffer.text_snapshot();
        let version = buffer.version();
        let tx = self.tx.clone();
        let workspace_root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        let path_label = display_path_label_cow(&path);
        self.status = format!("Saving {path_label}");
        self.record_async_task_started("File Save", path_label.into_owned());
        self.runtime.spawn(async move {
            if u64::try_from(text.len_bytes()).unwrap_or(u64::MAX) <= LOCAL_HISTORY_MAX_BYTES {
                let history_text = text.text();
                let _ = snapshot_file_before_save_async(
                    &workspace_root,
                    &path,
                    history_text.as_bytes(),
                    LOCAL_HISTORY_MAX_BYTES,
                )
                .await;
            }
            let result = write_text_snapshot_atomic_async(&path, text).await;
            match result {
                Ok(()) => {
                    let _ = crate::ui_event_channel::send_critical_ui_event(
                        &tx,
                        UiEvent::FileSaved {
                            root: workspace_root,
                            generation,
                            id,
                            path,
                            version,
                        },
                    );
                }
                Err(error) => {
                    let _ = crate::ui_event_channel::send_critical_ui_event(
                        &tx,
                        UiEvent::FileSaveFailed {
                            root: workspace_root,
                            generation,
                            id,
                            path,
                            error: error.to_string(),
                        },
                    );
                }
            }
        });
    }
}

pub(crate) fn apply_save_cleanup_for_buffer(
    buffer: &mut TextBuffer,
    trim_trailing_whitespace: bool,
    insert_final_newline: bool,
    trim_final_newlines: bool,
) -> bool {
    if buffer_uses_large_file_mode(buffer) {
        return false;
    }

    buffer.apply_save_cleanup(
        trim_trailing_whitespace,
        insert_final_newline,
        trim_final_newlines,
    )
}

#[cfg(test)]
mod tests {
    use super::apply_save_cleanup_for_buffer;
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        app_state::{PendingFileReload, PendingFormatOnSave, QueuedFileReload},
        large_file_mode::LARGE_FILE_MODE_MAX_BYTES,
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn save_cleanup_runs_for_normal_buffers() {
        let mut buffer = TextBuffer::from_text(1, None, "value  ".to_owned());

        assert!(apply_save_cleanup_for_buffer(&mut buffer, true, true, true));
        assert_eq!(buffer.text(), "value\n");
    }

    #[test]
    fn save_cleanup_skips_large_file_mode_buffers() {
        let mut text = "x".repeat(LARGE_FILE_MODE_MAX_BYTES + 1);
        text.push_str("  ");
        let mut buffer = TextBuffer::from_text(1, None, text.clone());

        assert!(!apply_save_cleanup_for_buffer(
            &mut buffer,
            true,
            true,
            true
        ));
        assert!(buffer.text_equals(&text));
    }

    #[test]
    fn save_to_blocks_read_only_untitled_buffers_before_writing() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::new_untitled(7);
        buffer.mark_dirty();
        buffer.set_read_only(true);
        app.buffers.push(buffer);

        app.spawn_save_to(7, root.join("saved.txt"));

        assert_eq!(app.status, "Cannot save Untitled; buffer is read-only");
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(!app.pending_format_on_save.contains_key(&7));
    }

    #[test]
    fn save_to_current_path_blocks_read_only_before_external_change_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        buffer.set_read_only(true);
        app.buffers.push(buffer);
        app.external_change_buffers.insert(7);
        app.close_after_save = Some(7);
        app.pending_close_buffers.push(8);

        app.spawn_save_to(7, path);

        assert_eq!(app.save_conflict_buffer, None);
        assert_eq!(app.close_after_save, None);
        assert_eq!(app.dirty_close_buffer, Some(7));
        assert_eq!(app.pending_close_buffers, vec![8]);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.external_change_buffers.contains(&7));
        assert!(app.status.contains("buffer is read-only"));
    }

    #[test]
    fn save_to_current_path_block_preserves_unrelated_dirty_close_guard() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other_path = root.join("src/other.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        buffer.set_read_only(true);
        let mut other = TextBuffer::from_text(9, Some(other_path), "other".to_owned());
        other.mark_dirty();
        app.buffers.push(buffer);
        app.buffers.push(other);
        app.close_after_save = Some(7);
        app.dirty_close_buffer = Some(9);
        app.pending_close_buffers.push(8);

        app.spawn_save_to(7, path);

        assert_eq!(app.close_after_save, None);
        assert_eq!(app.dirty_close_buffer, Some(9));
        assert_eq!(app.pending_close_buffers, vec![7, 8]);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.contains("buffer is read-only"));
    }

    #[test]
    fn save_to_current_path_blocks_binary_preview_before_external_change_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.binary_preview_buffers.insert(7);
        app.external_change_buffers.insert(7);

        app.spawn_save_to(7, path);

        assert_eq!(app.save_conflict_buffer, None);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.external_change_buffers.contains(&7));
        assert!(app.status.contains("binary previews are read-only"));
    }

    #[test]
    fn save_to_current_path_blocks_lossy_preview_before_external_change_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.lossy_decoded_buffers.insert(7);
        app.external_change_buffers.insert(7);

        app.spawn_save_to(7, path);

        assert_eq!(app.save_conflict_buffer, None);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.external_change_buffers.contains(&7));
        assert!(app.status.contains("replacement characters"));
    }

    #[test]
    fn save_to_over_external_change_still_blocks_read_only_before_writing() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        buffer.set_read_only(true);
        app.buffers.push(buffer);
        app.external_change_buffers.insert(7);

        app.spawn_save_to_over_external_change(7, path);

        assert_eq!(app.save_conflict_buffer, None);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.external_change_buffers.contains(&7));
        assert!(app.status.contains("buffer is read-only"));
    }

    #[test]
    fn save_to_current_path_with_pending_clean_reload_opens_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "clean".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path: path.clone(),
                version,
                force_dirty: false,
            },
        );

        app.spawn_save_to(7, path);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_to_current_path_with_queued_clean_reload_opens_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "clean".to_owned(),
        ));
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path: path.clone(),
                force_dirty: false,
            },
        );

        app.spawn_save_to(7, path);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_to_equivalent_current_path_with_queued_clean_reload_opens_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "clean".to_owned(),
        ));
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path,
                force_dirty: false,
            },
        );

        app.spawn_save_to(7, equivalent_path);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_to_copy_path_ignores_queued_clean_reload_for_current_path() {
        let root = PathBuf::from("workspace");
        let current_path = root.join("src/main.rs");
        let copy_path = root.join("src/copy.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(current_path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path: current_path,
                force_dirty: false,
            },
        );

        app.spawn_save_to(7, copy_path.clone());

        assert_eq!(app.save_conflict_buffer, None);
        assert!(app.in_flight_saves.contains(&7));
        assert!(!app.queued_save_paths.contains_key(&7));
        assert!(app.queued_file_reloads.contains_key(&7));
        assert!(app.status.starts_with("Saving "));
        assert!(app.status.contains("copy.rs"));
    }

    #[test]
    fn save_to_equivalent_current_path_with_external_change_marker_opens_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(7, Some(path), "clean".to_owned()));
        app.external_change_buffers.insert(7);

        app.spawn_save_to(7, equivalent_path);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_to_equivalent_current_path_with_pending_clean_reload_opens_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "clean".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path,
                version,
                force_dirty: false,
            },
        );

        app.spawn_save_to(7, equivalent_path);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_to_equivalent_pending_format_path_queues_existing_format_on_save() {
        let root = PathBuf::from("workspace");
        let save_path = root.join("src/main.rs");
        let current_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(current_path.clone()), "dirty".to_owned());
        let version = buffer.version();
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: save_path.clone(),
                format_path: save_path.clone(),
                version,
                request_id: 21,
            },
        );

        app.spawn_save_to(7, save_path.clone());

        assert_eq!(
            app.pending_format_on_save.get(&7),
            Some(&PendingFormatOnSave {
                save_path,
                format_path: current_path,
                version,
                request_id: 21,
            })
        );
        assert!(!app.canceled_formatting_request_ids.contains(&21));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.starts_with("Queued save "));
    }

    #[test]
    fn taking_canceled_formatting_request_removes_order_entry() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);

        app.cancel_lsp_formatting_request(21);
        assert!(app.canceled_formatting_request_ids.contains(&21));
        assert_eq!(
            app.canceled_formatting_request_order
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![21]
        );

        assert!(app.take_canceled_lsp_formatting_request(21));

        assert!(!app.canceled_formatting_request_ids.contains(&21));
        assert!(app.canceled_formatting_request_order.is_empty());
    }

    #[test]
    fn taking_unknown_canceled_formatting_request_keeps_order_entries() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);

        app.cancel_lsp_formatting_request(21);

        assert!(!app.take_canceled_lsp_formatting_request(22));
        assert!(app.canceled_formatting_request_ids.contains(&21));
        assert_eq!(
            app.canceled_formatting_request_order
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![21]
        );
    }

    #[test]
    fn format_on_save_overwrite_marker_helpers_are_idempotent() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);

        assert!(!app.format_on_save_overwrites_external_change(7));
        assert!(app.mark_format_on_save_overwrite_external_change(7));
        assert!(app.format_on_save_overwrites_external_change(7));
        assert!(!app.mark_format_on_save_overwrite_external_change(7));
        assert!(app.take_format_on_save_overwrite_external_change(7));
        assert!(!app.format_on_save_overwrites_external_change(7));
        assert!(!app.take_format_on_save_overwrite_external_change(7));
    }

    #[test]
    fn cancel_pending_format_on_save_clears_overwrite_marker() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.begin_pending_format_on_save(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path,
                version: 1,
                request_id: 21,
            },
        );
        app.mark_format_on_save_overwrite_external_change(7);

        app.cancel_pending_format_on_save(7);

        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(app.canceled_formatting_request_ids.contains(&21));
        assert!(!app.format_on_save_overwrites_external_change(7));
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
