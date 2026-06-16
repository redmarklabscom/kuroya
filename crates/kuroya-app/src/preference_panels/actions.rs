use crate::{
    KuroyaApp,
    path_display::display_error_label_cow,
    ui_event_channel::send_critical_ui_event,
    ui_events::{SettingsFontTarget, UiEvent},
};

use super::font_files::choose_font_file;

#[derive(Default)]
pub(super) struct PendingSettingsPanelActions {
    pub(super) close: bool,
    pub(super) apply: bool,
    pub(super) reset: bool,
    pub(super) reload: bool,
    pub(super) choose_editor_font: bool,
    pub(super) clear_editor_font: bool,
    pub(super) choose_ui_font: bool,
    pub(super) clear_ui_font: bool,
}

impl KuroyaApp {
    pub(super) fn apply_settings_panel_actions(&mut self, actions: PendingSettingsPanelActions) {
        if actions.close {
            self.settings_panel_open = false;
            self.sync_settings_panel_inputs();
            self.status = "Closed settings".to_owned();
        } else if actions.apply {
            self.apply_settings_panel();
        } else if actions.reset {
            self.reset_settings_panel_draft();
        } else if actions.reload {
            self.reload_settings();
        } else if actions.choose_editor_font {
            self.choose_settings_font(SettingsFontTarget::Editor);
        } else if actions.clear_editor_font {
            self.settings_editor_font_path.clear();
            self.status = "Cleared editor font file".to_owned();
        } else if actions.choose_ui_font {
            self.choose_settings_font(SettingsFontTarget::Ui);
        } else if actions.clear_ui_font {
            self.settings_ui_font_path.clear();
            self.status = "Cleared UI font file".to_owned();
        }
    }

    fn reset_settings_panel_draft(&mut self) {
        if !self.settings_panel_open {
            self.status = "Open settings before resetting draft".to_owned();
            return;
        }

        let had_pending_inputs = self.settings_panel_draft_validation().has_pending_inputs();
        self.sync_settings_panel_inputs();
        self.status = if had_pending_inputs {
            "Reset settings draft".to_owned()
        } else {
            "Settings already match the current configuration".to_owned()
        };
    }

    fn choose_settings_font(&mut self, target: SettingsFontTarget) {
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        let current = match target {
            SettingsFontTarget::Editor => self.settings_editor_font_path.clone(),
            SettingsFontTarget::Ui => self.settings_ui_font_path.clone(),
        };
        let tx = self.tx.clone();
        self.status = match target {
            SettingsFontTarget::Editor => "Choose an editor font file".to_owned(),
            SettingsFontTarget::Ui => "Choose a UI font file".to_owned(),
        };
        self.runtime.spawn_blocking(move || {
            let event = match choose_font_file(&root, &current) {
                Ok(Some(path)) => UiEvent::SettingsFontPicked {
                    root,
                    generation,
                    target,
                    path,
                },
                Ok(None) => UiEvent::SettingsFontPickerCanceled {
                    root,
                    generation,
                    target,
                },
                Err(error) => UiEvent::SettingsFontPickerFailed {
                    root,
                    generation,
                    target,
                    error,
                },
            };
            let _ = send_critical_ui_event(&tx, event);
        });
    }

    pub(crate) fn apply_settings_font_picked(&mut self, target: SettingsFontTarget, path: String) {
        match target {
            SettingsFontTarget::Editor => {
                self.settings_editor_font_path = path;
                self.status = "Selected editor font file".to_owned();
            }
            SettingsFontTarget::Ui => {
                self.settings_ui_font_path = path;
                self.status = "Selected UI font file".to_owned();
            }
        }
    }

    pub(crate) fn apply_settings_font_picker_canceled(&mut self, target: SettingsFontTarget) {
        self.status = match target {
            SettingsFontTarget::Editor => "Editor font selection canceled".to_owned(),
            SettingsFontTarget::Ui => "UI font selection canceled".to_owned(),
        };
    }

    pub(crate) fn apply_settings_font_picker_failed(
        &mut self,
        target: SettingsFontTarget,
        error: String,
    ) {
        self.status = font_selection_failure_status(target, &error);
    }
}

fn font_selection_failure_status(target: SettingsFontTarget, error: &str) -> String {
    let error = display_error_label_cow(error);
    let label = match target {
        SettingsFontTarget::Editor => "editor",
        SettingsFontTarget::Ui => "UI",
    };
    format!("Could not select {label} font file: {}", error.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{PendingSettingsPanelActions, font_selection_failure_status};
    use crate::{
        KuroyaApp, app_startup_context::AppStartupContext,
        path_display::DISPLAY_ERROR_LABEL_MAX_CHARS, terminal::TerminalPane,
        ui_events::SettingsFontTarget,
    };
    use kuroya_core::{EditorSettings, Workspace};
    use std::{
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn preferences_font_selection_failure_status_sanitizes_and_bounds_error_detail() {
        let status = font_selection_failure_status(
            SettingsFontTarget::Editor,
            &format!(
                "first line\nsecond line \u{202e}{}",
                "font-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
            ),
        );

        assert!(status.starts_with("Could not select editor font file: first line "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not select editor font file: ".chars().count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn preferences_font_selection_failure_status_falls_back_for_blank_error_detail() {
        assert_eq!(
            font_selection_failure_status(SettingsFontTarget::Ui, "\n\u{202e}\u{0007}"),
            "Could not select UI font file: unknown error"
        );
    }

    #[test]
    fn reset_settings_panel_draft_rejects_closed_panel_draft() {
        let root = temp_root("closed-reset-draft");
        let mut app = app_for_test(root.clone(), EditorSettings::default());
        app.settings_panel_open = false;
        app.settings_panel_draft.font_size = 18.0;

        app.reset_settings_panel_draft();

        assert_eq!(app.settings_panel_draft.font_size, 18.0);
        assert_eq!(app.status, "Open settings before resetting draft");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn closing_settings_panel_discards_stale_draft_inputs() {
        let root = temp_root("close-discards-draft");
        let settings = EditorSettings {
            font_size: 14.0,
            editor_font_path: Some("fonts/current.ttf".to_owned()),
            ..EditorSettings::default()
        };
        let mut app = app_for_test(root.clone(), settings.clone());
        app.settings_panel_open = true;
        app.settings_panel_draft.font_size = 22.0;
        app.settings_editor_font_path = "fonts/stale.ttf".to_owned();

        app.apply_settings_panel_actions(PendingSettingsPanelActions {
            close: true,
            ..PendingSettingsPanelActions::default()
        });

        assert!(!app.settings_panel_open);
        assert_eq!(app.settings_panel_draft.font_size, settings.font_size);
        assert_eq!(app.settings_editor_font_path, "fonts/current.ttf");
        assert_eq!(app.status, "Closed settings");
        let _ = std::fs::remove_dir_all(root);
    }

    fn app_for_test(root: PathBuf, settings: EditorSettings) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
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
        std::env::temp_dir().join(format!(
            "kuroya-actions-settings-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
