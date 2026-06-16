use crate::{
    KuroyaApp,
    explorer::{ExplorerEntryKind, path_matches_kind, retarget_path_prefix},
};
#[cfg(test)]
use kuroya_core::TextBuffer;
use std::path::Path;

impl KuroyaApp {
    pub(crate) fn retarget_explorer_open_buffers(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        kind: ExplorerEntryKind,
    ) -> usize {
        let mut retargeted = 0;
        for index in 0..self.buffers.len() {
            let update = {
                let buffer = &self.buffers[index];
                let Some(path) = buffer.path() else {
                    continue;
                };
                if path_matches_kind(path, old_path, kind) {
                    let updated_path = if kind == ExplorerEntryKind::File {
                        new_path.to_path_buf()
                    } else {
                        let Some(updated_path) = retarget_path_prefix(path, old_path, new_path)
                        else {
                            continue;
                        };
                        updated_path
                    };
                    Some((buffer.id(), self.diagnostic_path_for(buffer), updated_path))
                } else {
                    None
                }
            };
            let Some((id, old_diagnostic_path, updated_path)) = update else {
                continue;
            };

            self.buffers[index].set_path(updated_path);
            self.diagnostics.replace(old_diagnostic_path, Vec::new());
            self.diff_cache.remove(&id);
            self.diff_cache_pending.retain(|key, _| key.buffer_id != id);
            if !self.buffers[index].is_dirty() {
                self.clear_buffer_changed_on_disk(id);
            }
            self.spawn_diagnostics_for(id);
            self.notify_lsp_open(id);
            retargeted += 1;
        }

        retargeted
    }

    pub(crate) fn close_deleted_explorer_open_buffers(
        &mut self,
        path: &Path,
        kind: ExplorerEntryKind,
    ) -> (usize, usize) {
        let mut closed = 0;
        let mut retained_dirty = 0;
        let mut index = 0;
        while index < self.buffers.len() {
            let affected = self.buffers[index]
                .path()
                .is_some_and(|candidate| path_matches_kind(candidate, path, kind));
            if !affected {
                index += 1;
                continue;
            }

            let id = self.buffers[index].id();
            if self.buffers[index].is_dirty() {
                retained_dirty += 1;
                self.mark_buffer_changed_on_disk(id);
                index += 1;
            } else {
                self.force_close_buffer(id);
                closed += 1;
            }
        }
        (closed, retained_dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn retarget_explorer_open_buffers_preserves_dirty_external_change_marker() {
        let root = PathBuf::from("workspace");
        let old_path = root.join("src/main.rs");
        let new_path = root.join("src/lib.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(old_path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.mark_buffer_changed_on_disk(7);
        let generation = app.external_change_generation;

        assert_eq!(
            app.retarget_explorer_open_buffers(&old_path, &new_path, ExplorerEntryKind::File),
            1
        );

        assert_eq!(app.buffer(7).and_then(TextBuffer::path), Some(&new_path));
        assert!(app.buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, generation);
    }

    #[test]
    fn retarget_explorer_open_buffers_clears_clean_external_change_marker() {
        let root = PathBuf::from("workspace");
        let old_path = root.join("src/main.rs");
        let new_path = root.join("src/lib.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(old_path.clone()),
            "clean".to_owned(),
        ));
        app.mark_buffer_changed_on_disk(7);
        let generation = app.external_change_generation;

        assert_eq!(
            app.retarget_explorer_open_buffers(&old_path, &new_path, ExplorerEntryKind::File),
            1
        );

        assert_eq!(app.buffer(7).and_then(TextBuffer::path), Some(&new_path));
        assert!(!app.buffer_changed_on_disk(7));
        assert_eq!(app.external_change_generation, generation + 1);
    }

    #[test]
    fn retarget_explorer_open_buffers_matches_lexically_equivalent_file_path() {
        let root = PathBuf::from("workspace");
        let old_path = root.join("src").join("main.rs");
        let stored_path = root.join("src").join("..").join("src").join("main.rs");
        let new_path = root.join("src").join("lib.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(stored_path),
            "clean".to_owned(),
        ));

        assert_eq!(
            app.retarget_explorer_open_buffers(&old_path, &new_path, ExplorerEntryKind::File),
            1
        );

        assert_eq!(app.buffer(7).and_then(TextBuffer::path), Some(&new_path));
    }

    #[test]
    fn retarget_explorer_open_buffers_retargets_lexically_equivalent_folder_child() {
        let root = PathBuf::from("workspace");
        let old_path = root.join("src");
        let stored_path = root
            .join("src")
            .join("..")
            .join("src")
            .join("nested")
            .join("mod.rs");
        let new_path = root.join("crates").join("app").join("src");
        let expected = new_path.join("nested").join("mod.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(stored_path),
            "clean".to_owned(),
        ));

        assert_eq!(
            app.retarget_explorer_open_buffers(&old_path, &new_path, ExplorerEntryKind::Folder),
            1
        );

        assert_eq!(app.buffer(7).and_then(TextBuffer::path), Some(&expected));
    }

    #[test]
    fn close_deleted_explorer_open_buffers_marks_retained_dirty_buffers_changed_on_disk() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);

        let (closed, retained_dirty) =
            app.close_deleted_explorer_open_buffers(&path, ExplorerEntryKind::File);

        assert_eq!((closed, retained_dirty), (0, 1));
        assert!(app.buffer(7).is_some());
        assert!(app.buffer_changed_on_disk(7));
    }

    #[test]
    fn close_deleted_explorer_open_buffers_marks_dirty_lexical_folder_child_changed_on_disk() {
        let root = PathBuf::from("workspace");
        let deleted_path = root.join("src").join("..").join("src");
        let buffer_path = root.join("src").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(buffer_path), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);

        let (closed, retained_dirty) =
            app.close_deleted_explorer_open_buffers(&deleted_path, ExplorerEntryKind::Folder);

        assert_eq!((closed, retained_dirty), (0, 1));
        assert!(app.buffer(7).is_some());
        assert!(app.buffer_changed_on_disk(7));
    }

    #[test]
    fn close_deleted_explorer_open_buffers_closes_clean_lexical_folder_child() {
        let root = PathBuf::from("workspace");
        let deleted_path = root.join("src");
        let buffer_path = root.join("src").join("..").join("src").join("main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(buffer_path),
            "clean".to_owned(),
        ));

        let (closed, retained_dirty) =
            app.close_deleted_explorer_open_buffers(&deleted_path, ExplorerEntryKind::Folder);

        assert_eq!((closed, retained_dirty), (1, 0));
        assert!(app.buffer(7).is_none());
        assert!(!app.buffer_changed_on_disk(7));
    }

    #[test]
    fn deleted_open_file_state_can_be_persisted_during_shutdown() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.active = Some(7);
        app.panes[0].active = Some(7);

        let (closed, retained_dirty) =
            app.close_deleted_explorer_open_buffers(&path, ExplorerEntryKind::File);
        let session = app.build_session();
        app.terminal.close_all_sessions_for_shutdown();

        assert_eq!((closed, retained_dirty), (0, 1));
        assert!(app.buffer(7).is_some());
        assert!(app.buffer_changed_on_disk(7));
        assert_eq!(session.open_files, vec![path]);
        assert_eq!(session.active_path, Some(root.join("src/main.rs")));
    }

    #[test]
    fn deleted_open_folder_state_can_be_persisted_during_shutdown() {
        let root = PathBuf::from("workspace");
        let folder = root.join("src");
        let clean_path = folder.join("clean.rs");
        let dirty_path = folder.join("dirty.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(clean_path),
            "clean".to_owned(),
        ));
        let mut dirty = TextBuffer::from_text(8, Some(dirty_path.clone()), "dirty".to_owned());
        dirty.mark_dirty();
        app.buffers.push(dirty);
        app.active = Some(8);
        app.panes[0].active = Some(8);

        let (closed, retained_dirty) =
            app.close_deleted_explorer_open_buffers(&folder, ExplorerEntryKind::Folder);
        let session = app.build_session();
        app.terminal.close_all_sessions_for_shutdown();

        assert_eq!((closed, retained_dirty), (1, 1));
        assert!(app.buffer(7).is_none());
        assert!(app.buffer(8).is_some());
        assert!(app.buffer_changed_on_disk(8));
        assert_eq!(session.open_files, vec![dirty_path]);
        assert_eq!(session.active_path, Some(root.join("src/dirty.rs")));
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
