use crate::{
    KuroyaApp,
    fs_watcher::FileWatcher,
    ui_text::count_label,
    workspace_state::{
        classify_watched_paths, dirty_open_buffers_for_changes, reloadable_open_buffers_for_changes,
    },
    workspace_trust::{trusted_workspace_paths_match, workspace_path_stays_within_root_lexically},
};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

impl KuroyaApp {
    pub(crate) fn drain_file_watcher(&mut self) -> usize {
        let watcher_root_stale = self.watcher.as_ref().is_some_and(|watcher| {
            !watcher_root_matches_workspace(watcher.root(), &self.workspace.root)
        });
        if watcher_root_stale {
            self.watcher = FileWatcher::new(&self.workspace.root).ok();
            return 0;
        }

        let Some(watcher) = self.watcher.as_ref() else {
            return 0;
        };
        let drain = watcher.drain();
        let mut changed = drain.paths;
        dedupe_watcher_paths(&mut changed);
        let mut open_buffer_paths_added = 0;
        if drain.overflowed {
            open_buffer_paths_added =
                include_open_buffer_paths(&mut changed, &self.buffers, &self.workspace.root);
        }
        let changed_count = changed.len() + usize::from(drain.overflowed);
        if changed.is_empty() && !drain.overflowed {
            return 0;
        }

        let watched = classify_watched_paths(&self.workspace.root, &changed);
        if watched.settings_changed || drain.overflowed {
            self.reload_settings();
        }
        if watched.tasks_changed || drain.overflowed {
            self.spawn_workspace_task_load();
        }
        if watched.plugins_changed || drain.overflowed {
            self.schedule_workspace_plugin_reload();
        }
        if watched.project_paths.is_empty() && !drain.overflowed {
            return changed_count;
        }

        let reloads = reloadable_open_buffers_for_changes(&watched.project_paths, &self.buffers);
        for (id, path) in reloads {
            self.spawn_reload_clean_buffer(id, path);
        }

        let dirty_changes = dirty_open_buffers_for_changes(&watched.project_paths, &self.buffers);
        for (id, _) in &dirty_changes {
            self.mark_buffer_changed_on_disk(*id);
        }

        self.status = file_watcher_status(
            drain.overflowed,
            open_buffer_paths_added,
            watched.project_paths.len(),
            dirty_changes.len(),
        );
        if watched.workspace_refresh_needed || drain.overflowed {
            self.schedule_workspace_refresh();
        }
        changed_count
    }
}

fn file_watcher_status(
    overflowed: bool,
    open_buffer_paths_added: usize,
    project_changes: usize,
    dirty_changes: usize,
) -> String {
    if overflowed {
        return format!(
            "Filesystem watcher missed changes; refreshing workspace and checking {}",
            count_label(
                open_buffer_paths_added,
                "open buffer path",
                "open buffer paths"
            )
        );
    }

    let changes = count_label(project_changes, "filesystem change", "filesystem changes");
    if dirty_changes == 0 {
        format!("{changes} detected")
    } else {
        format!(
            "{changes} detected; {} changed on disk",
            count_label(dirty_changes, "dirty buffer", "dirty buffers")
        )
    }
}

fn include_open_buffer_paths(
    changed: &mut Vec<PathBuf>,
    buffers: &[kuroya_core::TextBuffer],
    workspace_root: &Path,
) -> usize {
    let mut seen = HashSet::with_capacity(changed.len().saturating_add(buffers.len()));
    seen.extend(changed.iter().map(|path| watcher_path_key(path)));
    let mut added = 0;
    for path in buffers
        .iter()
        .filter_map(kuroya_core::TextBuffer::path)
        .filter(|path| workspace_path_stays_within_root_lexically(workspace_root, path))
    {
        let path = path.to_path_buf();
        if seen.insert(watcher_path_key(&path)) {
            changed.push(path);
            added += 1;
        }
    }
    added
}

fn dedupe_watcher_paths(changed: &mut Vec<PathBuf>) -> usize {
    let original_len = changed.len();
    let mut seen = HashSet::with_capacity(changed.len());
    changed.retain(|path| seen.insert(watcher_path_key(path)));
    original_len.saturating_sub(changed.len())
}

fn watcher_root_matches_workspace(watcher_root: &Path, workspace_root: &Path) -> bool {
    trusted_workspace_paths_match(watcher_root, workspace_root)
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct WatcherPathKey {
    prefix: Option<String>,
    rooted: bool,
    components: Vec<String>,
}

fn watcher_path_key(path: &Path) -> WatcherPathKey {
    let mut key = WatcherPathKey {
        prefix: None,
        rooted: false,
        components: Vec::new(),
    };

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                key.prefix = Some(watcher_path_component_key(prefix.as_os_str()));
            }
            Component::RootDir => {
                key.rooted = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if key
                    .components
                    .last()
                    .is_some_and(|component| component != "..")
                {
                    key.components.pop();
                } else if !key.rooted {
                    key.components.push("..".to_owned());
                }
            }
            Component::Normal(component) => {
                key.components.push(watcher_path_component_key(component))
            }
        }
    }

    key
}

fn watcher_path_component_key(component: &OsStr) -> String {
    let component = component.to_string_lossy();
    #[cfg(windows)]
    {
        if component.is_ascii() {
            let mut component = component.into_owned();
            component.make_ascii_lowercase();
            component
        } else {
            component.to_lowercase()
        }
    }
    #[cfg(not(windows))]
    {
        component.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        dedupe_watcher_paths, file_watcher_status, include_open_buffer_paths,
        watcher_root_matches_workspace,
    };
    use kuroya_core::TextBuffer;
    use std::path::{Path, PathBuf};

    #[test]
    fn overflow_open_buffer_paths_are_deduplicated() {
        let workspace = Path::new("workspace");
        let main = PathBuf::from("workspace/src/main.rs");
        let lib = PathBuf::from("workspace/src/lib.rs");
        let mut changed = vec![main.clone()];
        let buffers = vec![
            TextBuffer::from_text(1, Some(main.clone()), "main".to_owned()),
            TextBuffer::from_text(2, Some(lib.clone()), "lib".to_owned()),
            TextBuffer::from_text(3, Some(lib.clone()), "duplicate".to_owned()),
            TextBuffer::new_untitled(4),
        ];

        let added = include_open_buffer_paths(&mut changed, &buffers, workspace);

        assert_eq!(added, 1);
        assert_eq!(changed, vec![main, lib]);
    }

    #[test]
    fn overflow_open_buffer_paths_deduplicate_lexically_equivalent_paths() {
        let workspace = Path::new("workspace");
        let raw_main = PathBuf::from("workspace/src/./main.rs");
        let open_main = PathBuf::from("workspace/src/main.rs");
        let mut changed = vec![raw_main.clone()];
        let buffers = vec![
            TextBuffer::from_text(1, Some(open_main), "main".to_owned()),
            TextBuffer::from_text(
                2,
                Some(PathBuf::from("workspace/src/generated/../main.rs")),
                "same".to_owned(),
            ),
        ];

        let added = include_open_buffer_paths(&mut changed, &buffers, workspace);

        assert_eq!(added, 0);
        assert_eq!(changed, vec![raw_main]);
    }

    #[test]
    fn watcher_paths_are_deduplicated_before_overflow_processing() {
        let raw_main = PathBuf::from("workspace/src/./main.rs");
        let equivalent_main = PathBuf::from("workspace/src/generated/../main.rs");
        let lib = PathBuf::from("workspace/src/lib.rs");
        let mut changed = vec![
            raw_main.clone(),
            equivalent_main,
            lib.clone(),
            PathBuf::from("workspace/src/./lib.rs"),
        ];

        assert_eq!(dedupe_watcher_paths(&mut changed), 2);
        assert_eq!(changed, vec![raw_main, lib]);
    }

    #[test]
    fn overflow_open_buffer_paths_skip_buffers_outside_workspace() {
        let workspace = Path::new("workspace");
        let main = PathBuf::from("workspace/src/main.rs");
        let outside = PathBuf::from("other/src/lib.rs");
        let mut changed = Vec::new();
        let buffers = vec![
            TextBuffer::from_text(1, Some(main.clone()), "main".to_owned()),
            TextBuffer::from_text(2, Some(outside), "outside".to_owned()),
        ];

        let added = include_open_buffer_paths(&mut changed, &buffers, workspace);

        assert_eq!(added, 1);
        assert_eq!(changed, vec![main]);
    }

    #[test]
    fn overflow_open_buffer_paths_skip_parent_reentry_paths() {
        let workspace = Path::new("workspace/current");
        let reentry = PathBuf::from("workspace/current/../current/src/main.rs");
        let mut changed = Vec::new();
        let buffers = vec![TextBuffer::from_text(1, Some(reentry), "main".to_owned())];

        let added = include_open_buffer_paths(&mut changed, &buffers, workspace);

        assert_eq!(added, 0);
        assert!(changed.is_empty());
    }

    #[test]
    fn watcher_root_match_rejects_stale_parent_or_child_roots() {
        assert!(watcher_root_matches_workspace(
            Path::new("workspace/src/.."),
            Path::new("workspace")
        ));
        assert!(!watcher_root_matches_workspace(
            Path::new("workspace/old"),
            Path::new("workspace")
        ));
        assert!(!watcher_root_matches_workspace(
            Path::new("workspace"),
            Path::new("workspace/current")
        ));
    }

    #[test]
    fn file_watcher_status_uses_count_labels() {
        assert_eq!(
            file_watcher_status(false, 0, 1, 0),
            "1 filesystem change detected"
        );
        assert_eq!(
            file_watcher_status(false, 0, 2, 0),
            "2 filesystem changes detected"
        );
        assert_eq!(
            file_watcher_status(false, 0, 1, 1),
            "1 filesystem change detected; 1 dirty buffer changed on disk"
        );
        assert_eq!(
            file_watcher_status(false, 0, 2, 2),
            "2 filesystem changes detected; 2 dirty buffers changed on disk"
        );
        assert_eq!(
            file_watcher_status(true, 1, 0, 0),
            "Filesystem watcher missed changes; refreshing workspace and checking 1 open buffer path"
        );
        assert_eq!(
            file_watcher_status(true, 2, 0, 0),
            "Filesystem watcher missed changes; refreshing workspace and checking 2 open buffer paths"
        );
    }
}
