use crate::{
    KuroyaApp,
    git_diff_state::{DiffCacheEntry, DiffCacheRequestKey, GIT_DIFF_MAX_BYTES},
    navigation_targets::normalize_changed_line_kinds_for_buffer,
    path_display::compact_path,
    ui_events::UiEvent,
    workspace_state::{paths_match_lexically, workspace_event_matches},
};
use kuroya_core::{
    BufferId, DiffOptions, GitLineChangeKind, changed_line_kinds_against_head_with_options,
};
use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

const DIFF_CACHE_PENDING_RETRY_AFTER: Duration = Duration::from_secs(2);

impl KuroyaApp {
    pub(crate) fn diff_lines_for(
        &mut self,
        buffer_id: BufferId,
    ) -> BTreeMap<usize, GitLineChangeKind> {
        let (line_count, path, version) = {
            let Some(buffer) = self.buffer(buffer_id) else {
                return BTreeMap::new();
            };
            if buffer.len_bytes() > GIT_DIFF_MAX_BYTES {
                return BTreeMap::new();
            }
            let line_count = buffer.len_lines().max(1);
            let Some(path) = buffer.path().cloned() else {
                return BTreeMap::new();
            };
            (line_count, path, buffer.version())
        };
        let ignore_trim_whitespace = self
            .settings
            .scm_diff_decorations_ignore_trim_whitespace
            .resolve(self.settings.diff_ignore_trim_whitespace);
        let root = self.workspace.root.clone();
        let request_key = DiffCacheRequestKey {
            root: root.clone(),
            buffer_id,
            path: path.clone(),
            version,
            ignore_trim_whitespace,
        };
        if let Some(cache) = self.diff_cache.get(&buffer_id)
            && cache.version == version
            && cache.ignore_trim_whitespace == ignore_trim_whitespace
        {
            return cache.lines.clone();
        }
        let now = Instant::now();
        if diff_cache_pending_blocks_request(
            &mut self.diff_cache_pending,
            &request_key,
            now,
            DIFF_CACHE_PENDING_RETRY_AFTER,
        ) {
            return BTreeMap::new();
        }

        let Some(snapshot) = self
            .buffer(buffer_id)
            .filter(|buffer| buffer.version() == version)
            .map(|buffer| buffer.text_snapshot())
        else {
            return BTreeMap::new();
        };
        let tx = self.tx.clone();
        self.diff_cache_pending.insert(request_key, now);
        self.record_async_task_started("Editor Diff Lines", compact_path(&path));
        self.runtime.spawn_blocking(move || {
            let text = snapshot.text();
            let lines = normalize_changed_line_kinds_for_buffer(
                changed_line_kinds_against_head_with_options(
                    &root,
                    &path,
                    &text,
                    20_000,
                    DiffOptions {
                        ignore_trim_whitespace,
                        ..DiffOptions::default()
                    },
                )
                .unwrap_or_default(),
                line_count,
            );
            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::EditorDiffLinesComputed {
                    root,
                    id: buffer_id,
                    path,
                    version,
                    ignore_trim_whitespace,
                    lines,
                },
            );
        });

        BTreeMap::new()
    }

    pub(crate) fn diff_lines_pending_for(&self, buffer_id: BufferId) -> bool {
        let Some(path) = self.buffer(buffer_id).and_then(|buffer| buffer.path()) else {
            return false;
        };
        self.diff_cache_pending.keys().any(|key| {
            key.buffer_id == buffer_id
                && workspace_event_matches(&self.workspace.root, &key.root)
                && paths_match_lexically(path, &key.path)
        })
    }

    pub(crate) fn apply_editor_diff_lines_computed(
        &mut self,
        root: std::path::PathBuf,
        id: BufferId,
        path: std::path::PathBuf,
        version: u64,
        ignore_trim_whitespace: bool,
        lines: BTreeMap<usize, GitLineChangeKind>,
    ) {
        let request_key = DiffCacheRequestKey {
            root: root.clone(),
            buffer_id: id,
            path: path.clone(),
            version,
            ignore_trim_whitespace,
        };
        remove_diff_cache_pending_request(&mut self.diff_cache_pending, &request_key);
        if !workspace_event_matches(&self.workspace.root, &root)
            || !self.buffer(id).is_some_and(|buffer| {
                buffer.version() == version
                    && buffer
                        .path()
                        .is_some_and(|candidate| paths_match_lexically(candidate, &path))
            })
        {
            return;
        }

        self.diff_cache.insert(
            id,
            DiffCacheEntry {
                version,
                ignore_trim_whitespace,
                lines,
            },
        );
    }
}

fn diff_cache_pending_blocks_request(
    pending: &mut HashMap<DiffCacheRequestKey, Instant>,
    request_key: &DiffCacheRequestKey,
    now: Instant,
    retry_after: Duration,
) -> bool {
    pending.retain(|key, queued_at| {
        key.buffer_id != request_key.buffer_id
            || (diff_cache_request_targets_match(key, request_key)
                && now.saturating_duration_since(*queued_at) < retry_after)
    });
    pending.keys().any(|key| {
        key.buffer_id == request_key.buffer_id && diff_cache_request_targets_match(key, request_key)
    })
}

fn remove_diff_cache_pending_request(
    pending: &mut HashMap<DiffCacheRequestKey, Instant>,
    request_key: &DiffCacheRequestKey,
) {
    pending.retain(|key, _| {
        key.buffer_id != request_key.buffer_id
            || key.version != request_key.version
            || key.ignore_trim_whitespace != request_key.ignore_trim_whitespace
            || !diff_cache_request_targets_match(key, request_key)
    });
}

fn diff_cache_request_targets_match(
    left: &DiffCacheRequestKey,
    right: &DiffCacheRequestKey,
) -> bool {
    workspace_event_matches(&left.root, &right.root)
        && paths_match_lexically(&left.path, &right.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn editor_diff_lines_result_updates_matching_buffer_cache() {
        let root = std::env::temp_dir().join("kuroya-editor-diff-lines-cache-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "new\n".to_owned(),
        ));
        let version = app.buffer(7).unwrap().version();
        let lines = BTreeMap::from([(1, GitLineChangeKind::Added)]);
        let request_key = diff_key(&root, 7, &path, version, false);
        app.diff_cache_pending
            .insert(request_key.clone(), Instant::now());

        app.apply_editor_diff_lines_computed(root, 7, path, version, false, lines.clone());

        assert!(!app.diff_cache_pending.contains_key(&request_key));
        let entry = app
            .diff_cache
            .get(&7)
            .expect("diff cache should be populated");
        assert_eq!(entry.version, version);
        assert!(!entry.ignore_trim_whitespace);
        assert_eq!(entry.lines, lines);
        assert_eq!(app.diff_lines_for(7), lines);
        assert!(app.diff_cache_pending.is_empty());
    }

    #[test]
    fn editor_diff_lines_result_accepts_equivalent_workspace_root() {
        let root = PathBuf::from("workspace/current");
        let event_root = root.join("src").join("..");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "new\n".to_owned(),
        ));
        let version = app.buffer(7).unwrap().version();
        let lines = BTreeMap::from([(1, GitLineChangeKind::Added)]);
        app.diff_cache_pending
            .insert(diff_key(&root, 7, &path, version, false), Instant::now());

        app.apply_editor_diff_lines_computed(event_root, 7, path, version, false, lines.clone());

        assert!(app.diff_cache_pending.is_empty());
        assert_eq!(app.diff_cache.get(&7).unwrap().lines, lines);
    }

    #[test]
    fn editor_diff_lines_result_accepts_equivalent_buffer_path() {
        let root = PathBuf::from("workspace/current");
        let path = root.join("src/main.rs");
        let event_path = root.join("src").join("..").join("src").join("main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "new\n".to_owned(),
        ));
        let version = app.buffer(7).unwrap().version();
        let lines = BTreeMap::from([(1, GitLineChangeKind::Added)]);
        app.diff_cache_pending
            .insert(diff_key(&root, 7, &path, version, false), Instant::now());

        app.apply_editor_diff_lines_computed(root, 7, event_path, version, false, lines.clone());

        assert!(app.diff_cache_pending.is_empty());
        assert_eq!(app.diff_cache.get(&7).unwrap().lines, lines);
    }

    #[test]
    fn editor_diff_lines_result_ignores_stale_buffer_version() {
        let root = std::env::temp_dir().join("kuroya-editor-diff-lines-stale-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            9,
            Some(path.clone()),
            "new\n".to_owned(),
        ));
        let stale_version = app.buffer(9).unwrap().version();
        app.buffer_mut(9).unwrap().insert_at_cursor("dirty");
        app.diff_cache_pending.insert(
            diff_key(&root, 9, &path, stale_version, true),
            Instant::now(),
        );

        app.apply_editor_diff_lines_computed(
            root,
            9,
            path,
            stale_version,
            true,
            BTreeMap::from([(1, GitLineChangeKind::Modified)]),
        );

        assert!(app.diff_cache_pending.is_empty());
        assert!(!app.diff_cache.contains_key(&9));
    }

    #[test]
    fn editor_diff_lines_stale_workspace_result_keeps_current_pending_request() {
        let root = PathBuf::from("workspace/current");
        let old_root = PathBuf::from("workspace/old");
        let path = root.join("src/main.rs");
        let old_path = old_root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            9,
            Some(path.clone()),
            "new\n".to_owned(),
        ));
        let version = app.buffer(9).unwrap().version();
        let current_key = diff_key(&root, 9, &path, version, true);
        app.diff_cache_pending
            .insert(current_key.clone(), Instant::now());

        app.apply_editor_diff_lines_computed(
            old_root,
            9,
            old_path,
            version,
            true,
            BTreeMap::from([(1, GitLineChangeKind::Modified)]),
        );

        assert!(app.diff_cache_pending.contains_key(&current_key));
        assert!(!app.diff_cache.contains_key(&9));
    }

    #[test]
    fn editor_diff_lines_stale_path_result_keeps_current_pending_request() {
        let root = PathBuf::from("workspace/current");
        let path = root.join("src/main.rs");
        let old_path = root.join("src/old.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            9,
            Some(path.clone()),
            "new\n".to_owned(),
        ));
        let version = app.buffer(9).unwrap().version();
        let current_key = diff_key(&root, 9, &path, version, true);
        app.diff_cache_pending
            .insert(current_key.clone(), Instant::now());

        app.apply_editor_diff_lines_computed(
            root,
            9,
            old_path,
            version,
            true,
            BTreeMap::from([(1, GitLineChangeKind::Modified)]),
        );

        assert!(app.diff_cache_pending.contains_key(&current_key));
        assert!(!app.diff_cache.contains_key(&9));
    }

    #[test]
    fn editor_diff_lines_pending_request_blocks_same_buffer_bursts() {
        let now = Instant::now();
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut pending = HashMap::from([(diff_key(&root, 7, &path, 1, false), now)]);

        assert!(diff_cache_pending_blocks_request(
            &mut pending,
            &diff_key(&root, 7, &path, 2, false),
            now + Duration::from_millis(50),
            DIFF_CACHE_PENDING_RETRY_AFTER,
        ));
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn editor_diff_lines_pending_request_allows_changed_path() {
        let now = Instant::now();
        let root = PathBuf::from("workspace");
        let old_path = root.join("src/old.rs");
        let new_path = root.join("src/main.rs");
        let mut pending = HashMap::from([(diff_key(&root, 7, &old_path, 1, false), now)]);

        assert!(!diff_cache_pending_blocks_request(
            &mut pending,
            &diff_key(&root, 7, &new_path, 1, false),
            now + Duration::from_millis(50),
            DIFF_CACHE_PENDING_RETRY_AFTER,
        ));
        assert!(pending.is_empty());
    }

    #[test]
    fn editor_diff_lines_pending_request_retries_after_timeout() {
        let now = Instant::now();
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut pending = HashMap::from([(diff_key(&root, 7, &path, 1, false), now)]);

        assert!(!diff_cache_pending_blocks_request(
            &mut pending,
            &diff_key(&root, 7, &path, 1, false),
            now + DIFF_CACHE_PENDING_RETRY_AFTER + Duration::from_millis(1),
            DIFF_CACHE_PENDING_RETRY_AFTER,
        ));
        assert!(pending.is_empty());
    }

    fn diff_key(
        root: &std::path::Path,
        buffer_id: BufferId,
        path: &std::path::Path,
        version: u64,
        ignore_trim_whitespace: bool,
    ) -> DiffCacheRequestKey {
        DiffCacheRequestKey {
            root: root.to_path_buf(),
            buffer_id,
            path: path.to_path_buf(),
            version,
            ignore_trim_whitespace,
        }
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
