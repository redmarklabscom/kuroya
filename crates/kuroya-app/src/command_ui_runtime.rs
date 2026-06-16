use crate::{
    KuroyaApp,
    panel_layout::cycle_panel_placement,
    path_display::{display_error_label_cow, display_path_label_cow},
    theme::selected_theme_index_with_plugins,
    workspace_state::settings_path,
};
use kuroya_core::{Command, EditorFindSeedSearchStringFromSelection, TextBuffer};
use std::{
    fmt::{Display, Write},
    path::Path,
};

impl KuroyaApp {
    pub(crate) fn prepare_terminal_open_height(&mut self) {
        if !self.terminal.visible {
            self.terminal_open_height_pending = true;
        }
    }

    pub(crate) fn set_terminal_panel_visible(&mut self, visible: bool) {
        if visible {
            self.prepare_terminal_open_height();
        } else {
            self.terminal_open_height_pending = false;
        }
        self.terminal.set_visible(visible);
    }

    fn toggle_terminal_panel(&mut self) {
        self.set_terminal_panel_visible(!self.terminal.visible);
    }

    pub(crate) fn run_ui_command(&mut self, command: &Command) -> bool {
        if command_ui_requires_git(command) && !self.settings.git_enabled {
            self.close_git_ui_overlays();
            self.status = "Git is disabled".to_owned();
            return true;
        }

        match command {
            Command::ReloadSettings => self.reload_settings(),
            Command::CheckForUpdates => self.check_for_updates(),
            Command::OpenSettingsFile => self.open_settings_file(),
            Command::ToggleSettingsPanel => {
                self.settings_panel_open = !self.settings_panel_open;
                if self.settings_panel_open {
                    self.sync_settings_panel_inputs();
                }
            }
            Command::ToggleKeybindingsPanel => {
                self.keybindings_open = !self.keybindings_open;
                self.keybindings_query.clear();
                self.keybindings_selected = 0;
                self.keybinding_capture_command = None;
            }
            Command::ToggleThemePicker => {
                self.theme_picker_open = !self.theme_picker_open;
                self.theme_picker_selected =
                    selected_theme_index_with_plugins(&self.settings.theme, &self.plugin_themes);
            }
            Command::CycleTheme => self.cycle_theme(),
            Command::ToggleMinimap => {
                self.settings.minimap = !self.settings.minimap;
                self.settings_panel_draft.minimap = self.settings.minimap;
                self.save_toggled_editor_setting("Minimap", self.settings.minimap);
            }
            Command::ToggleStickyScroll => {
                self.settings.sticky_scroll = !self.settings.sticky_scroll;
                self.settings_panel_draft.sticky_scroll = self.settings.sticky_scroll;
                self.save_toggled_editor_setting("Sticky Scroll", self.settings.sticky_scroll);
            }
            Command::TrustWorkspace => self.trust_current_workspace(),
            Command::RevokeWorkspaceTrust => self.revoke_current_workspace_trust(),
            Command::ToggleCommandPalette => {
                self.toggle_command_palette();
            }
            Command::ToggleQuickOpen => {
                self.toggle_quick_open();
            }
            Command::ToggleBufferFind => {
                if self.buffer_find_open {
                    self.buffer_find_open = false;
                    self.buffer_find_scope = None;
                } else {
                    self.buffer_find_open = true;
                    self.buffer_find_match = 0;
                    self.buffer_find_scope = self.capture_active_find_scope();
                }
                if self.buffer_find_open
                    && let Some(selected) = find_query_seed_from_selection(
                        self.active_buffer(),
                        self.settings.find_seed_search_string_from_selection,
                    )
                {
                    self.buffer_find_query = selected;
                    self.buffer_find_query_history_cursor = None;
                    self.buffer_find_query_history_draft = None;
                }
            }
            Command::ToggleGoToLine => {
                if self.goto_line_open {
                    self.goto_line_open = false;
                } else {
                    self.begin_goto_line();
                }
            }
            Command::ToggleSymbolsPanel => {
                self.symbols_panel = !self.symbols_panel;
                if self.symbols_panel {
                    self.request_lsp_document_symbols();
                }
            }
            Command::CycleSymbolsPanelPlacement => {
                self.status = cycle_panel_placement(
                    &mut self.symbols_panel,
                    &mut self.symbols_panel_placement,
                    "File Symbols",
                );
                self.request_lsp_document_symbols();
            }
            Command::ToggleWorkspaceSymbols => {
                if self.workspace_symbols_open {
                    self.close_workspace_symbols(true);
                } else {
                    self.close_command_palette();
                    self.close_quick_open();
                    self.begin_workspace_symbols();
                }
            }
            Command::ToggleWorkspaceTasks => {
                if self.workspace_tasks_open {
                    self.workspace_tasks_open = false;
                    self.status = "Closed workspace tasks".to_owned();
                } else {
                    self.begin_workspace_tasks();
                }
            }
            Command::ToggleProjectSearch => {
                self.project_search = !self.project_search;
                if self.project_search {
                    self.project_search_selected = 0;
                }
            }
            Command::CycleProjectSearchPlacement => {
                self.status = cycle_panel_placement(
                    &mut self.project_search,
                    &mut self.project_search_placement,
                    "Project Search",
                );
                self.project_search_selected = 0;
            }
            Command::ToggleDiagnosticsPanel => {
                self.diagnostics_panel = !self.diagnostics_panel;
            }
            Command::CycleDiagnosticsPanelPlacement => {
                self.status = cycle_panel_placement(
                    &mut self.diagnostics_panel,
                    &mut self.diagnostics_panel_placement,
                    "Diagnostics",
                );
            }
            Command::ToggleDevtools => {
                self.devtools_open = !self.devtools_open;
            }
            Command::ToggleSourceControl => {
                self.source_control = !self.source_control;
                if self.source_control {
                    self.source_control_selected = 0;
                }
            }
            Command::CycleSourceControlPlacement => {
                self.status = cycle_panel_placement(
                    &mut self.source_control,
                    &mut self.source_control_placement,
                    "Source Control",
                );
                self.source_control_selected = 0;
            }
            Command::ToggleGitBranchSwitcher => {
                if self.source_control_branch_picker_open {
                    self.source_control_branch_picker_open = false;
                } else {
                    self.begin_git_branch_switcher();
                }
            }
            Command::ToggleGitHistory => {
                if self.source_control_history_open {
                    self.source_control_history_open = false;
                } else {
                    self.begin_git_history_panel();
                }
            }
            Command::ToggleGitStashes => {
                if self.source_control_stashes_open {
                    self.source_control_stashes_open = false;
                } else {
                    self.begin_git_stashes_panel();
                }
            }
            Command::OpenSourceControlInIntegratedTerminal => {
                let root = self.workspace.root.clone();
                let status = source_control_terminal_opened_status(&root);
                self.prepare_terminal_open_height();
                self.terminal.open_new_session_at(root);
                self.status = status;
            }
            Command::ToggleTerminal => self.toggle_terminal_panel(),
            Command::ToggleTerminalSearch => {
                if self.terminal.visible {
                    self.terminal.toggle_terminal_search();
                } else {
                    self.set_terminal_panel_visible(true);
                    self.terminal.open_terminal_search();
                }
            }
            Command::NextTerminalSearchResult => {
                self.set_terminal_panel_visible(true);
                self.terminal.next_terminal_search_result();
            }
            Command::PreviousTerminalSearchResult => {
                self.set_terminal_panel_visible(true);
                self.terminal.previous_terminal_search_result();
            }
            Command::NextTerminalSession => {
                self.prepare_terminal_open_height();
                self.terminal.activate_relative_session(1);
                self.status = "Focused next terminal session".to_owned();
            }
            Command::PreviousTerminalSession => {
                self.prepare_terminal_open_height();
                self.terminal.activate_relative_session(-1);
                self.status = "Focused previous terminal session".to_owned();
            }
            _ => return false,
        }
        true
    }

    fn toggle_command_palette(&mut self) {
        if self.command_palette {
            self.close_command_palette();
        } else {
            self.close_quick_open();
            self.close_workspace_symbols(false);
            self.command_palette = true;
            self.command_query.clear();
            self.command_selected = 0;
        }
    }

    pub(crate) fn close_command_palette(&mut self) {
        self.command_palette = false;
        self.command_query.clear();
        self.command_selected = 0;
        self.command_palette_results_cache = None;
    }

    fn toggle_quick_open(&mut self) {
        if self.quick_open {
            self.close_quick_open();
        } else {
            self.close_command_palette();
            self.close_workspace_symbols(false);
            self.quick_open = true;
            self.quick_open_query.clear();
            self.quick_open_selected = 0;
        }
    }

    fn close_quick_open(&mut self) {
        self.quick_open = false;
        self.quick_open_query.clear();
        self.quick_open_selected = 0;
    }

    fn close_workspace_symbols(&mut self, update_status: bool) {
        self.workspace_symbols_open = false;
        self.workspace_symbol_query.clear();
        self.workspace_symbol_submitted_query.clear();
        self.workspace_symbol_submitted_path = None;
        self.workspace_symbols.clear();
        self.workspace_symbols_selected = 0;
        if update_status {
            self.status = "Closed workspace symbols".to_owned();
        }
    }

    fn close_git_ui_overlays(&mut self) {
        self.source_control_branch_picker_open = false;
        self.source_control_history_open = false;
        self.source_control_stashes_open = false;
    }

    fn save_toggled_editor_setting(&mut self, label: &str, enabled: bool) {
        match self.settings.save(&settings_path(&self.workspace.root)) {
            Ok(()) => {
                let state = if enabled { "enabled" } else { "disabled" };
                let mut status = String::with_capacity(label.len() + 1 + state.len());
                status.push_str(label);
                status.push(' ');
                status.push_str(state);
                self.status = status;
            }
            Err(error) => {
                self.status = editor_setting_save_failure_status(label, error);
            }
        }
    }
}

fn source_control_terminal_opened_status(root: &Path) -> String {
    let root = display_path_label_cow(root);
    let mut status = String::with_capacity("Opened Source Control terminal at ".len() + root.len());
    status.push_str("Opened Source Control terminal at ");
    status.push_str(&root);
    status
}

fn editor_setting_save_failure_status(label: &str, error: impl Display) -> String {
    let error = error.to_string();
    let error = display_error_label_cow(&error);
    let mut status = String::with_capacity(
        label.len() + " changed, but settings save failed: ".len() + error.len(),
    );
    let _ = write!(status, "{label} changed, but settings save failed: {error}");
    status
}

fn command_ui_requires_git(command: &Command) -> bool {
    matches!(
        command,
        Command::ToggleGitBranchSwitcher
            | Command::ToggleGitHistory
            | Command::ToggleGitStashes
            | Command::OpenSourceControlInIntegratedTerminal
    )
}

pub(crate) fn find_query_seed_from_selection(
    buffer: Option<&TextBuffer>,
    mode: EditorFindSeedSearchStringFromSelection,
) -> Option<String> {
    let buffer = buffer?;
    if buffer.has_selection() {
        return find_query_seed_from_selected_text(buffer, mode);
    }
    mode.seeds_word_at_cursor()
        .then(|| buffer.word_at_cursor())
        .flatten()
}

fn find_query_seed_from_selected_text(
    buffer: &TextBuffer,
    mode: EditorFindSeedSearchStringFromSelection,
) -> Option<String> {
    mode.seeds_selection()
        .then(|| buffer.selected_text())
        .flatten()
        .filter(|text| !text.contains('\n'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        command_palette_overlay::CommandPaletteResultsCache,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, Workspace};
    use std::{
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn editor_setting_toggle_commands_persist_and_sync_panel_draft() {
        let root = temp_root("editor-toggle-commands");
        let settings = EditorSettings {
            minimap: true,
            sticky_scroll: true,
            ..EditorSettings::default()
        };
        let mut app = app_for_test(root.clone(), settings);

        assert!(app.run_ui_command(&Command::ToggleMinimap));
        assert!(!app.settings.minimap);
        assert!(!app.settings_panel_draft.minimap);
        assert_eq!(app.status, "Minimap disabled");
        let saved = std::fs::read_to_string(settings_path(&root)).expect("settings should save");
        assert!(saved.contains("minimap = false"));

        assert!(app.run_ui_command(&Command::ToggleStickyScroll));
        assert!(!app.settings.sticky_scroll);
        assert!(!app.settings_panel_draft.sticky_scroll);
        assert_eq!(app.status, "Sticky Scroll disabled");
        let saved = std::fs::read_to_string(settings_path(&root)).expect("settings should save");
        assert!(saved.contains("sticky_scroll = false"));
    }

    #[test]
    fn editor_setting_toggle_commands_preserve_unrelated_panel_draft_edits() {
        let root = temp_root("editor-toggle-draft-preservation");
        let settings = EditorSettings {
            minimap: true,
            sticky_scroll: true,
            font_size: 14.0,
            ..EditorSettings::default()
        };
        let mut app = app_for_test(root, settings);
        app.settings_panel_open = true;
        app.settings_panel_draft.font_size = 19.0;

        assert!(app.run_ui_command(&Command::ToggleMinimap));

        assert!(!app.settings.minimap);
        assert!(!app.settings_panel_draft.minimap);
        assert_eq!(app.settings.font_size, 14.0);
        assert_eq!(app.settings_panel_draft.font_size, 19.0);
    }

    #[test]
    fn source_control_terminal_opened_status_sanitizes_and_bounds_path_label() {
        let root = PathBuf::from("workspace").join(format!(
            "project\n{}\u{202e}tail",
            "very-long-component-".repeat(16)
        ));

        let status = source_control_terminal_opened_status(&root);

        assert!(status.starts_with("Opened Source Control terminal at project "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Opened Source Control terminal at ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn editor_setting_save_failure_status_sanitizes_and_bounds_error_detail() {
        let status = editor_setting_save_failure_status(
            "Minimap",
            format!("first line\n{}\u{202e}tail", "error-detail-".repeat(24)),
        );

        assert!(status.starts_with("Minimap changed, but settings save failed: first line "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Minimap changed, but settings save failed: "
                    .chars()
                    .count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn editor_setting_save_failure_status_falls_back_for_blank_error_detail() {
        assert_eq!(
            editor_setting_save_failure_status("Sticky Scroll", "\n\u{202e}\u{0007}"),
            "Sticky Scroll changed, but settings save failed: unknown error"
        );
    }

    #[test]
    fn command_palette_open_closes_other_command_pickers_and_clears_stale_state() {
        let root = temp_root("command-palette-picker-state");
        let mut app = app_for_test(root.clone(), EditorSettings::default());
        app.quick_open = true;
        app.quick_open_query = "main".to_owned();
        app.quick_open_selected = 9;
        app.workspace_symbols_open = true;
        app.workspace_symbol_query = "task".to_owned();
        app.workspace_symbol_submitted_query = "task".to_owned();
        app.workspace_symbol_submitted_path = Some(root.join("src/main.rs"));
        app.workspace_symbols_selected = 7;

        assert!(app.run_ui_command(&Command::ToggleCommandPalette));

        assert!(app.command_palette);
        assert_eq!(app.command_query, "");
        assert_eq!(app.command_selected, 0);
        assert!(!app.quick_open);
        assert_eq!(app.quick_open_query, "");
        assert_eq!(app.quick_open_selected, 0);
        assert!(!app.workspace_symbols_open);
        assert_eq!(app.workspace_symbol_query, "");
        assert_eq!(app.workspace_symbol_submitted_query, "");
        assert_eq!(app.workspace_symbol_submitted_path, None);
        assert_eq!(app.workspace_symbols_selected, 0);
    }

    #[test]
    fn command_palette_close_clears_stale_query_selection_and_results_cache() {
        let root = temp_root("command-palette-close-state");
        let mut app = app_for_test(root, EditorSettings::default());
        app.command_palette = true;
        app.command_query = "git".to_owned();
        app.command_selected = 4;
        app.command_palette_results_cache = Some(CommandPaletteResultsCache::default());

        app.close_command_palette();

        assert!(!app.command_palette);
        assert_eq!(app.command_query, "");
        assert_eq!(app.command_selected, 0);
        assert!(app.command_palette_results_cache.is_none());
    }

    #[test]
    fn workspace_symbols_close_clears_stale_submitted_result_state() {
        let root = temp_root("workspace-symbol-close-state");
        let mut app = app_for_test(root.clone(), EditorSettings::default());
        app.workspace_symbols_open = true;
        app.workspace_symbol_query = "main".to_owned();
        app.workspace_symbol_submitted_query = "main".to_owned();
        app.workspace_symbol_submitted_path = Some(root.join("src/main.rs"));
        app.workspace_symbols_selected = 3;

        assert!(app.run_ui_command(&Command::ToggleWorkspaceSymbols));

        assert!(!app.workspace_symbols_open);
        assert_eq!(app.workspace_symbol_query, "");
        assert_eq!(app.workspace_symbol_submitted_query, "");
        assert_eq!(app.workspace_symbol_submitted_path, None);
        assert_eq!(app.workspace_symbols_selected, 0);
        assert_eq!(app.status, "Closed workspace symbols");
    }

    #[test]
    fn git_only_ui_commands_are_filtered_when_git_is_disabled() {
        let root = temp_root("git-disabled-ui-filter");
        let settings = EditorSettings {
            git_enabled: false,
            ..EditorSettings::default()
        };
        let mut app = app_for_test(root, settings);

        assert!(command_ui_requires_git(&Command::ToggleGitHistory));
        assert!(command_ui_requires_git(
            &Command::OpenSourceControlInIntegratedTerminal
        ));
        assert!(app.run_ui_command(&Command::ToggleGitHistory));

        assert!(!app.source_control_history_open);
        assert_eq!(app.status, "Git is disabled");
    }

    #[test]
    fn toggle_terminal_schedules_responsive_open_height() {
        let root = temp_root("terminal-open-height");
        let mut app = app_for_test(root, EditorSettings::default());

        assert!(!app.terminal.visible);
        assert!(!app.terminal_open_height_pending);

        assert!(app.run_ui_command(&Command::ToggleTerminal));

        assert!(app.terminal.visible);
        assert!(app.terminal_open_height_pending);
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
        std::env::temp_dir().join(format!("kuroya-{name}-{}-{nanos}", std::process::id()))
    }
}
