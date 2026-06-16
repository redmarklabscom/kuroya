use crate::{
    KuroyaApp,
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, sanitized_display_label_cow,
    },
    save_lifecycle::protected_preview_save_block_reason_for_buffer,
};
use kuroya_core::{BufferId, TextBuffer};
use std::{borrow::Cow, path::PathBuf};

mod save_all;

const FILE_SAVE_STATUS_MAX_CHARS: usize = 240;

enum SaveTarget {
    ExistingPath(PathBuf),
    SaveAs,
    ProtectedPreview(&'static str),
}

impl KuroyaApp {
    pub(crate) fn spawn_save(&mut self, id: BufferId) {
        let Some(target) = self.save_target_for_buffer(id) else {
            return;
        };

        let path = match target {
            SaveTarget::ExistingPath(path) => path,
            SaveTarget::SaveAs => {
                self.begin_save_as(id);
                return;
            }
            SaveTarget::ProtectedPreview(reason) => {
                self.block_protected_preview_save(id, reason);
                return;
            }
        };
        if self.save_needs_observed_external_change_confirmation(id, &path) {
            self.open_save_conflict_for_buffer(id);
            return;
        }

        self.spawn_save_to(id, path);
    }

    pub(crate) fn force_save_over_external_change(&mut self, id: BufferId) {
        let Some(target) = self.save_target_for_buffer(id) else {
            return;
        };

        let path = match target {
            SaveTarget::ExistingPath(path) => path,
            SaveTarget::SaveAs => {
                self.begin_save_as(id);
                return;
            }
            SaveTarget::ProtectedPreview(reason) => {
                self.block_protected_preview_save(id, reason);
                return;
            }
        };
        self.cancel_deferred_reload_work(id);
        self.spawn_save_to_over_external_change(id, path);
    }

    fn save_target_for_buffer(&self, id: BufferId) -> Option<SaveTarget> {
        let buffer = self.buffer(id)?;
        if let Some(reason) = protected_preview_save_block_reason_for_buffer(
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        ) {
            return Some(SaveTarget::ProtectedPreview(reason));
        }

        Some(match buffer.path() {
            Some(path) => SaveTarget::ExistingPath(path.to_path_buf()),
            None => SaveTarget::SaveAs,
        })
    }

    pub(crate) fn block_protected_preview_save(&mut self, id: BufferId, reason: &'static str) {
        if self.close_after_save == Some(id) {
            self.close_after_save = None;
            self.restore_dirty_close_guard_for_buffer(id);
        }
        let reason_label = display_error_label_cow(reason);
        self.status = file_save_status(format!(
            "Cannot save {}; {}",
            self.file_io_buffer_label(id),
            reason_label.as_ref()
        ));
    }

    pub(crate) fn block_protected_preview_edit(&mut self, id: BufferId) -> bool {
        let reason = crate::editor_input::protected_preview_edit_block_reason(
            id,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        )
        .map(Cow::Borrowed)
        .or_else(|| {
            self.settings.read_only.then(|| {
                Cow::Owned(crate::editor_readonly::configured_read_only_reason(
                    &self.settings.read_only_message,
                ))
            })
        })
        .or_else(|| {
            self.buffer(id)
                .is_some_and(TextBuffer::is_read_only)
                .then_some(Cow::Borrowed("buffer is read-only"))
        });
        let Some(reason) = reason else {
            return false;
        };
        let reason_label = display_error_label_cow(reason.as_ref());
        self.status = file_save_status(format!(
            "Cannot edit {}; {}",
            self.file_io_buffer_label(id),
            reason_label.as_ref()
        ));
        true
    }

    pub(crate) fn file_io_buffer_label(&self, id: BufferId) -> String {
        file_io_buffer_label_text(self.buffer_label(id))
    }

    pub(crate) fn restore_dirty_close_guard_for_buffer(&mut self, id: BufferId) {
        if !self.buffer(id).is_some_and(TextBuffer::is_dirty) {
            return;
        }

        match self.dirty_close_buffer {
            None => {
                self.dirty_close_buffer = Some(id);
                self.pending_close_buffers.retain(|pending| *pending != id);
            }
            Some(active_id) if active_id != id => {
                if !self.pending_close_buffers.contains(&id) {
                    self.pending_close_buffers.insert(0, id);
                }
            }
            Some(_) => {
                self.pending_close_buffers.retain(|pending| *pending != id);
            }
        }
    }
}

fn file_io_buffer_label_text(label: String) -> String {
    match sanitized_display_label_cow(&label, DISPLAY_PATH_LABEL_MAX_CHARS, "Untitled") {
        Cow::Borrowed(borrowed)
            if borrowed.as_ptr() == label.as_ptr() && borrowed.len() == label.len() =>
        {
            label
        }
        Cow::Borrowed(borrowed) => borrowed.to_owned(),
        Cow::Owned(label) => label,
    }
}

fn file_save_status(status: String) -> String {
    crate::ui_text::truncate_middle(&status, FILE_SAVE_STATUS_MAX_CHARS)
}

#[cfg(test)]
mod tests {
    use super::{FILE_SAVE_STATUS_MAX_CHARS, file_io_buffer_label_text, file_save_status};
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        app_state::{PendingFileReload, QueuedFileReload},
        path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, sanitized_display_label},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn file_io_buffer_label_reuses_clean_ascii_and_unicode_buffer_labels() {
        for raw in ["clean.rs", "clean-\u{03bb}.rs"] {
            let input = raw.to_owned();
            let input_ptr = input.as_ptr();

            let label = file_io_buffer_label_text(input);

            assert_eq!(label, raw);
            assert_eq!(label.as_ptr(), input_ptr);
        }
    }

    #[test]
    fn file_io_buffer_label_owns_dirty_truncated_and_fallback_output() {
        let long = format!("main-{}.rs", "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2));
        let labels = [
            " clean.rs ",
            "bad\nname\u{202e}",
            long.as_str(),
            "\n\u{202e}\u{0007}",
        ];

        for raw in labels {
            let input = raw.to_owned();

            let label = file_io_buffer_label_text(input);

            assert_eq!(
                label,
                sanitized_display_label(raw, DISPLAY_PATH_LABEL_MAX_CHARS, "Untitled")
            );
            assert!(!label.contains('\n'));
            assert!(!label.contains('\u{202e}'));
            assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        }
    }

    #[test]
    fn file_io_buffer_label_sanitizes_virtual_labels_for_status_text() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(7, None, "text".to_owned()));
        let raw_label = format!("bad\n{}\u{202e}.rs", "very-long-name-".repeat(16));
        app.virtual_buffer_labels.insert(7, raw_label.clone());

        let label = app.file_io_buffer_label(7);

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        assert_eq!(app.virtual_buffer_labels.get(&7), Some(&raw_label));
    }

    #[test]
    fn file_io_buffer_label_wrapper_matches_helper_and_status_text() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(7, None, "text".to_owned()));
        app.virtual_buffer_labels
            .insert(7, "status-\u{03bb}.rs".to_owned());
        let expected = file_io_buffer_label_text(app.buffer_label(7));

        assert_eq!(app.file_io_buffer_label(7), expected);

        app.block_protected_preview_save(7, "buffer is read-only");

        assert_eq!(
            app.status,
            file_save_status(format!("Cannot save {expected}; buffer is read-only"))
        );
    }

    #[test]
    fn file_save_status_bounds_composed_messages() {
        let status = file_save_status(format!(
            "Cannot save {}; {}",
            "bad ".repeat(FILE_SAVE_STATUS_MAX_CHARS),
            "error ".repeat(FILE_SAVE_STATUS_MAX_CHARS)
        ));

        assert!(status.chars().count() <= FILE_SAVE_STATUS_MAX_CHARS);
    }

    #[test]
    fn protected_edit_status_is_bounded_and_display_safe() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(7, None, "text".to_owned()));
        app.virtual_buffer_labels.insert(
            7,
            format!(
                "buffer\n{}\u{202e}",
                "very-long-label-".repeat(FILE_SAVE_STATUS_MAX_CHARS)
            ),
        );
        app.settings.read_only = true;
        app.settings.read_only_message = format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(FILE_SAVE_STATUS_MAX_CHARS * 2)
        );

        assert!(app.block_protected_preview_edit(7));

        assert!(app.status.starts_with("Cannot edit "));
        assert!(!app.status.contains('\n'));
        assert!(!app.status.contains('\u{202e}'));
        assert!(app.status.chars().count() <= FILE_SAVE_STATUS_MAX_CHARS);
    }

    #[test]
    fn protected_edit_status_and_save_status_preserve_protected_preview_behavior() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.lossy_decoded_buffers.insert(7);
        app.close_after_save = Some(7);

        app.spawn_save(7);

        assert_eq!(
            app.status,
            "Cannot save main.rs; file was decoded with replacement characters"
        );
        assert_eq!(app.close_after_save, None);
        assert_eq!(app.dirty_close_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert_eq!(app.save_conflict_buffer, None);

        assert!(app.block_protected_preview_edit(7));
        assert_eq!(
            app.status,
            "Cannot edit main.rs; UTF-8 replacement previews are read-only"
        );
    }

    #[test]
    fn manual_save_clean_buffer_with_external_change_marker_opens_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "clean".to_owned(),
        ));
        app.external_change_buffers.insert(7);

        app.spawn_save(7);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn manual_save_current_path_with_queued_clean_reload_opens_conflict() {
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
                path,
                force_dirty: false,
            },
        );

        app.spawn_save(7);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_and_close_with_pending_clean_reload_opens_conflict_and_preserves_close_request() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        let version = buffer.version();
        app.buffers.push(buffer);
        app.close_after_save = Some(7);
        app.pending_close_buffers.push(8);
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path,
                version,
                force_dirty: false,
            },
        );

        app.spawn_save(7);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert_eq!(app.close_after_save, Some(7));
        assert_eq!(app.pending_close_buffers, vec![8]);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_and_close_with_queued_clean_reload_opens_conflict_and_preserves_close_request() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.close_after_save = Some(7);
        app.pending_close_buffers.push(8);
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path,
                force_dirty: false,
            },
        );

        app.spawn_save(7);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert_eq!(app.close_after_save, Some(7));
        assert_eq!(app.pending_close_buffers, vec![8]);
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_all_treats_pending_clean_reload_as_external_change_blocker() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
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

        app.save_all_dirty_buffers();

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_all_treats_equivalent_pending_clean_reload_as_external_change_blocker() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), "dirty".to_owned());
        buffer.mark_dirty();
        let version = buffer.version();
        app.buffers.push(buffer);
        app.in_flight_reloads.insert(
            7,
            PendingFileReload {
                request_id: 1,
                path: equivalent_path,
                version,
                force_dirty: false,
            },
        );

        app.save_all_dirty_buffers();

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_all_treats_queued_pending_clean_reload_as_external_change_blocker() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path,
                force_dirty: false,
            },
        );

        app.save_all_dirty_buffers();

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn save_all_allows_force_dirty_queued_reload() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.queued_file_reloads.insert(
            7,
            QueuedFileReload {
                path,
                force_dirty: true,
            },
        );

        app.save_all_dirty_buffers();

        assert_eq!(app.save_conflict_buffer, None);
        assert!(app.in_flight_saves.contains(&7));
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
