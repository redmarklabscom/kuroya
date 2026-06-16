use crate::{KuroyaApp, large_file_mode::buffer_uses_large_file_mode};
use kuroya_core::{BufferId, MergeConflict};

#[derive(Debug, Clone)]
pub(crate) struct MergeConflictCacheEntry {
    pub(crate) version: u64,
    pub(crate) line_count: usize,
    pub(crate) conflicts: Vec<MergeConflict>,
}

impl MergeConflictCacheEntry {
    fn is_valid_for(&self, version: u64, line_count: usize) -> bool {
        self.version == version
            && self.line_count == line_count
            && valid_merge_conflict_ranges(&self.conflicts, line_count)
    }

    fn refresh(&mut self, version: u64, line_count: usize, conflicts: &[MergeConflict]) {
        self.version = version;
        self.line_count = line_count;
        self.conflicts.clear();
        self.conflicts.extend_from_slice(conflicts);
    }
}

impl KuroyaApp {
    pub(crate) fn merge_conflicts_for_buffer(
        &mut self,
        id: BufferId,
        buffer_index: usize,
    ) -> Vec<MergeConflict> {
        let Some(buffer) = self.buffers.get(buffer_index) else {
            self.merge_conflict_cache.remove(&id);
            return Vec::new();
        };
        if buffer.id() != id || buffer_uses_large_file_mode(buffer) {
            self.merge_conflict_cache.remove(&id);
            return Vec::new();
        }

        let version = buffer.version();
        let line_count = buffer.len_lines();
        if let Some(entry) = self.merge_conflict_cache.get(&id)
            && entry.is_valid_for(version, line_count)
        {
            return entry.conflicts.clone();
        }

        let conflicts = buffer.merge_conflicts();
        if let Some(entry) = self.merge_conflict_cache.get_mut(&id) {
            entry.refresh(version, line_count, &conflicts);
        } else {
            self.merge_conflict_cache.insert(
                id,
                MergeConflictCacheEntry {
                    version,
                    line_count,
                    conflicts: conflicts.clone(),
                },
            );
        }

        conflicts
    }

    pub(crate) fn clear_buffer_merge_conflict_cache(&mut self, id: BufferId) {
        self.merge_conflict_cache.remove(&id);
    }
}

fn valid_merge_conflict_ranges(conflicts: &[MergeConflict], line_count: usize) -> bool {
    let mut previous_end_line = None;
    for conflict in conflicts {
        if conflict.start_line >= conflict.separator_line
            || conflict.separator_line >= conflict.end_line
        {
            return false;
        }
        if conflict.end_line >= line_count {
            return false;
        }
        if let Some(previous_end_line) = previous_end_line
            && conflict.start_line <= previous_end_line
        {
            return false;
        }
        previous_end_line = Some(conflict.end_line);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn merge_conflicts_for_buffer_reuses_cached_version() {
        let root = std::env::temp_dir().join("kuroya-merge-conflict-cache-reuse-test");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            None,
            "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n".to_owned(),
        ));

        let initial = app.merge_conflicts_for_buffer(7, 0);
        assert_eq!(initial.len(), 1);
        app.merge_conflict_cache
            .get_mut(&7)
            .expect("entry should be cached")
            .conflicts
            .clear();

        assert!(app.merge_conflicts_for_buffer(7, 0).is_empty());
    }

    #[test]
    fn merge_conflicts_for_buffer_recomputes_stale_version() {
        let root = std::env::temp_dir().join("kuroya-merge-conflict-cache-stale-test");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(9, None, "plain\n".to_owned()));
        let current_version = app.buffers[0].version();
        app.merge_conflict_cache.insert(
            9,
            MergeConflictCacheEntry {
                version: current_version + 1,
                line_count: app.buffers[0].len_lines(),
                conflicts: vec![MergeConflict {
                    start_line: 0,
                    separator_line: 1,
                    end_line: 2,
                }],
            },
        );

        assert!(app.merge_conflicts_for_buffer(9, 0).is_empty());
        assert_eq!(
            app.merge_conflict_cache
                .get(&9)
                .expect("entry should be refreshed")
                .version,
            current_version
        );
    }

    #[test]
    fn merge_conflicts_for_buffer_skips_large_file_mode_and_clears_cache() {
        let root = std::env::temp_dir().join("kuroya-merge-conflict-cache-large-file-test");
        let mut app = app_for_test(root);
        let large_text = "x".repeat(crate::large_file_mode::LARGE_FILE_MODE_MAX_BYTES + 1);
        app.buffers
            .push(TextBuffer::from_text(11, None, large_text));
        app.merge_conflict_cache.insert(
            11,
            MergeConflictCacheEntry {
                version: 1,
                line_count: app.buffers[0].len_lines(),
                conflicts: vec![MergeConflict {
                    start_line: 0,
                    separator_line: 1,
                    end_line: 2,
                }],
            },
        );

        assert!(app.merge_conflicts_for_buffer(11, 0).is_empty());
        assert!(!app.merge_conflict_cache.contains_key(&11));
    }

    #[test]
    fn merge_conflicts_for_buffer_recomputes_invalid_same_version_ranges() {
        let root = std::env::temp_dir().join("kuroya-merge-conflict-cache-invalid-range-test");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            13,
            None,
            "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n".to_owned(),
        ));
        let current_version = app.buffers[0].version();
        app.merge_conflict_cache.insert(
            13,
            MergeConflictCacheEntry {
                version: current_version,
                line_count: app.buffers[0].len_lines(),
                conflicts: vec![MergeConflict {
                    start_line: 2,
                    separator_line: 1,
                    end_line: 4,
                }],
            },
        );

        assert_eq!(
            app.merge_conflicts_for_buffer(13, 0),
            vec![MergeConflict {
                start_line: 0,
                separator_line: 2,
                end_line: 4,
            }]
        );
    }

    #[test]
    fn merge_conflicts_for_buffer_recomputes_stale_same_version_line_count() {
        let root = std::env::temp_dir().join("kuroya-merge-conflict-cache-line-count-test");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            17,
            None,
            "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n".to_owned(),
        ));
        let current_version = app.buffers[0].version();
        app.merge_conflict_cache.insert(
            17,
            MergeConflictCacheEntry {
                version: current_version,
                line_count: app.buffers[0].len_lines() + 1,
                conflicts: Vec::new(),
            },
        );

        let conflicts = app.merge_conflicts_for_buffer(17, 0);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            app.merge_conflict_cache
                .get(&17)
                .expect("entry should be refreshed")
                .line_count,
            app.buffers[0].len_lines()
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
