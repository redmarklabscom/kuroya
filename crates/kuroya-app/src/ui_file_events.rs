use crate::{KuroyaApp, ui_events::UiEvent, workspace_state::workspace_event_matches};
use std::path::Path;

impl KuroyaApp {
    pub(crate) fn handle_file_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::FileLoaded { .. }
            | UiEvent::ImageFileLoaded { .. }
            | UiEvent::FileLoadFailed { .. } => self.handle_file_load_event(event),
            UiEvent::FileReloaded { .. }
            | UiEvent::ImageFileReloaded { .. }
            | UiEvent::FileReloadFailed { .. } => self.handle_file_reload_event(event),
            UiEvent::FileSaved { .. } | UiEvent::FileSaveFailed { .. } => {
                self.handle_file_save_event(event)
            }
            _ => {}
        }
    }

    pub(crate) fn workspace_event_is_current(&self, root: &Path, generation: u64) -> bool {
        generation == self.workspace_event_generation
            && workspace_event_matches(&self.workspace.root, root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn workspace_event_is_current_accepts_equivalent_root_and_current_generation() {
        let root = PathBuf::from("workspace");
        let event_root = root.join("src").join("..");
        let app = app_for_test(root);

        assert!(app.workspace_event_is_current(&event_root, app.workspace_event_generation));
    }

    #[test]
    fn workspace_event_is_current_rejects_other_root() {
        let app = app_for_test(PathBuf::from("workspace"));

        assert!(!app.workspace_event_is_current(
            Path::new("other-workspace"),
            app.workspace_event_generation,
        ));
    }

    #[test]
    fn workspace_event_is_current_rejects_stale_generation() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        let stale_generation = app.workspace_event_generation;
        app.reset_open_workspace_state();

        assert!(!app.workspace_event_is_current(&root, stale_generation));
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
