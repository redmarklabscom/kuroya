use crate::{
    KuroyaApp,
    app_session_restore::restore_session_workspace_root_matches,
    path_display::{display_error_label_cow, display_path_label_cow},
    persistence::{load_latest_workspace_snapshot, save_workspace_snapshot},
    ui_text::count_label,
};
use std::{borrow::Cow, path::Path};

impl KuroyaApp {
    pub(crate) fn save_workspace_snapshot_now(&mut self) {
        let session = self.build_session();
        match save_workspace_snapshot(&self.workspace.root, &session) {
            Ok(path) => {
                let path_label = workspace_snapshot_path_label_cow(&path);
                self.status = format!("Saved workspace snapshot {}", path_label.as_ref());
            }
            Err(error) => {
                let error = error.to_string();
                let error_detail = workspace_snapshot_error_detail_cow(&error);
                self.status = format!(
                    "Could not save workspace snapshot: {}",
                    error_detail.as_ref()
                );
            }
        }
    }

    pub(crate) fn restore_latest_workspace_snapshot(&mut self) {
        let dirty_count = self
            .buffers
            .iter()
            .filter(|buffer| buffer.is_dirty())
            .count();
        if dirty_count > 0 {
            self.status = workspace_snapshot_dirty_restore_status(dirty_count);
            return;
        }

        let loaded = match load_latest_workspace_snapshot(&self.workspace.root) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                self.status = "No workspace snapshots found".to_owned();
                return;
            }
            Err(error) => {
                let error = error.to_string();
                let error_detail = workspace_snapshot_error_detail_cow(&error);
                self.status = format!(
                    "Could not restore workspace snapshot: {}",
                    error_detail.as_ref()
                );
                return;
            }
        };
        if !restore_session_workspace_root_matches(
            &self.workspace.root,
            &loaded.session.workspace_root,
        ) {
            self.status = workspace_snapshot_mismatched_root_status();
            return;
        }

        let workspace_root = self.workspace.root.clone();
        self.notify_lsp_close_all();
        for client in self.lsp_clients.values() {
            client.shutdown();
        }
        let snapshot_path = loaded.path;
        self.reset_workspace_lsp_clients();
        self.reset_open_workspace_state();
        self.restore_session(loaded.session);
        self.request_session_save(workspace_root, self.build_session_save_snapshot());
        self.spawn_index();
        self.spawn_git_scan();
        self.spawn_workspace_task_load();
        self.spawn_plugin_discovery();
        let snapshot_path_label = workspace_snapshot_path_label_cow(&snapshot_path);
        self.status = format!(
            "Restored workspace snapshot {}",
            snapshot_path_label.as_ref()
        );
    }
}

#[cfg(test)]
pub(crate) fn workspace_snapshot_path_label(path: &Path) -> String {
    workspace_snapshot_path_label_cow(path).into_owned()
}

fn workspace_snapshot_path_label_cow(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

#[cfg(test)]
fn workspace_snapshot_error_detail(error: &str) -> String {
    workspace_snapshot_error_detail_cow(error).into_owned()
}

fn workspace_snapshot_error_detail_cow(error: &str) -> Cow<'_, str> {
    display_error_label_cow(error)
}

fn workspace_snapshot_dirty_restore_status(dirty_count: usize) -> String {
    format!(
        "Save or close {} before restoring workspace snapshot",
        count_label(dirty_count, "dirty file", "dirty files")
    )
}

fn workspace_snapshot_mismatched_root_status() -> String {
    "Could not restore workspace snapshot: snapshot workspace root does not match current workspace"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        persistence::{PersistedSession, RecoveredBuffer, save_workspace_snapshot},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, Workspace};
    use std::{
        fs,
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn workspace_snapshot_restore_queues_restored_session_when_save_is_in_flight() {
        let root = temp_root("workspace-snapshot-restore-queue");
        fs::create_dir_all(&root).unwrap();
        let restored_path = root.join("src/main.rs");
        let restored = PersistedSession {
            workspace_root: root.clone(),
            open_files: vec![restored_path.clone()],
            active_path: Some(restored_path.clone()),
            pane_paths: vec![Some(restored_path.clone())],
            source_control_commit_message: "restored commit draft".to_owned(),
            recent_projects: vec![root.clone()],
            recovery: vec![RecoveredBuffer {
                path: Some(restored_path),
                display_name: "main.rs".to_owned(),
                text: "restored dirty text".to_owned(),
            }],
            ..PersistedSession::default()
        };
        save_workspace_snapshot(&root, &restored).unwrap();

        let mut app = app_for_test(root.clone());
        app.source_control_commit_message = "pre-restore commit draft".to_owned();
        app.session_save_in_flight = Some(root.clone());
        app.queued_session_saves
            .insert(root.clone(), app.build_session_save_snapshot());

        app.restore_latest_workspace_snapshot();

        let queued = app
            .queued_session_saves
            .get(&root)
            .cloned()
            .expect("restored session should be queued")
            .into_persisted_session();
        assert_eq!(
            queued.source_control_commit_message,
            "restored commit draft"
        );
        assert_eq!(queued.active_path, restored.active_path);
        assert_eq!(queued.recovery.len(), 1);
        assert_eq!(queued.recovery[0].text, "restored dirty text");
        assert_ne!(
            queued.source_control_commit_message,
            "pre-restore commit draft"
        );

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_snapshot_restore_preserves_current_raw_workspace_root() {
        let root = temp_root("workspace-snapshot-raw-root");
        fs::create_dir_all(root.join("child")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        let raw_root = root.join("child").join("..");
        let restored_path = root.join("src/main.rs");
        let restored = PersistedSession {
            workspace_root: root.clone(),
            open_files: vec![restored_path.clone()],
            active_path: Some(restored_path),
            pane_paths: vec![None],
            ..PersistedSession::default()
        };
        save_workspace_snapshot(&raw_root, &restored).unwrap();

        let mut app = app_for_test(raw_root.clone());

        app.restore_latest_workspace_snapshot();

        assert_eq!(app.workspace.root, raw_root);
        assert!(app.status.starts_with("Restored workspace snapshot "));

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_snapshot_path_label_is_single_line_and_bounded() {
        let root = PathBuf::from(format!(
            "workspace/snapshot\n{}\u{202e}.json",
            "x".repeat(200)
        ));
        let label = workspace_snapshot_path_label(&root);

        assert!(!label.chars().any(char::is_control));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn workspace_snapshot_error_detail_is_single_line_and_bounded() {
        let error = workspace_snapshot_error_detail(&format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(400)
        ));

        assert!(!error.chars().any(char::is_control));
        assert!(!error.contains('\u{202e}'));
        assert!(error.contains("..."));
        assert!(error.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    #[test]
    fn workspace_snapshot_dirty_restore_status_uses_file_count_labels() {
        assert_eq!(
            workspace_snapshot_dirty_restore_status(1),
            "Save or close 1 dirty file before restoring workspace snapshot"
        );
        assert_eq!(
            workspace_snapshot_dirty_restore_status(2),
            "Save or close 2 dirty files before restoring workspace snapshot"
        );
    }

    #[test]
    fn workspace_snapshot_mismatched_root_status_is_specific() {
        assert_eq!(
            workspace_snapshot_mismatched_root_status(),
            "Could not restore workspace snapshot: snapshot workspace root does not match current workspace"
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

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!("kuroya-{name}-{}-{nanos}", std::process::id()))
    }
}
