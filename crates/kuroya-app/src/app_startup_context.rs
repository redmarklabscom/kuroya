use crate::{
    devtools_startup::{StartupProfiler, StartupTimingEntry},
    editor_vim_key_events::sanitize_vim_settings_for_runtime,
    fonts::{apply_typography, install_fonts},
    fs_watcher::FileWatcher,
    persistence::{AppState, PersistedSession},
    preferences::load_workspace_settings,
    settings_form::optional_setting_path_to_input,
    terminal::TerminalPane,
    theme::apply_theme,
    theme_picker_panel::selected_theme_picker_index_for_settings,
    ui_event_channel::{Receiver, Sender, ui_event_channel},
    ui_events::UiEvent,
    workspace_state::paths_match_lexically,
    workspace_trust::workspace_is_trusted,
};
use anyhow::Context as _;
use kuroya_core::{EditorSettings, PluginThemeRegistry, Workspace, window_zoom_factor};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::runtime::Runtime;

const EMPTY_STARTUP_WORKSPACE_DIR_NAME: &str = "empty-workspace";

pub(crate) struct AppStartupContext {
    pub(crate) runtime: Runtime,
    pub(crate) tx: Sender<UiEvent>,
    pub(crate) rx: Receiver<UiEvent>,
    pub(crate) workspace: Workspace,
    pub(crate) settings: EditorSettings,
    pub(crate) settings_panel_draft: EditorSettings,
    pub(crate) settings_editor_font_path: String,
    pub(crate) settings_ui_font_path: String,
    pub(crate) theme_picker_selected: usize,
    pub(crate) saved_session: Option<PersistedSession>,
    pub(crate) terminal: TerminalPane,
    pub(crate) watcher: Option<FileWatcher>,
    pub(crate) recent_projects: Vec<PathBuf>,
    pub(crate) trusted_workspaces: Vec<PathBuf>,
    pub(crate) now: Instant,
    pub(crate) startup_timings: Vec<StartupTimingEntry>,
}

impl AppStartupContext {
    pub(crate) fn load(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let mut startup_profiler = StartupProfiler::start(Instant::now());
        let runtime = create_runtime()?;
        let (tx, rx) = ui_event_channel();
        let workspace_root = empty_startup_workspace_root();
        let _ = fs::create_dir_all(&workspace_root);
        let workspace = Workspace::new(workspace_root);
        startup_profiler.record("Initialize runtime");

        let app_state = AppState::load().unwrap_or_default();
        let workspace_trusted =
            workspace_is_trusted(&app_state.trusted_workspaces, &workspace.root);
        let mut settings = load_workspace_settings(&workspace.root, workspace_trusted)
            .map(|loaded| loaded.settings)
            .unwrap_or_default();
        if !workspace_trusted {
            apply_restricted_app_state_vim_settings(&mut settings, &app_state);
        }
        startup_profiler.record("Load settings");

        install_fonts(&cc.egui_ctx, &workspace.root, &settings);
        apply_typography(&cc.egui_ctx, &settings);
        apply_theme(&cc.egui_ctx, &settings.theme);
        cc.egui_ctx
            .set_zoom_factor(window_zoom_factor(settings.window_zoom_level));
        startup_profiler.record("Configure UI");

        let theme_picker_selected = selected_theme_picker_index_for_settings(
            &workspace.root,
            &settings,
            &PluginThemeRegistry::default(),
        );
        let settings_panel_draft = settings.clone();
        let settings_editor_font_path = optional_setting_path_to_input(&settings.editor_font_path);
        let settings_ui_font_path = optional_setting_path_to_input(&settings.ui_font_path);
        let saved_session = None;
        startup_profiler.record("Load persistence");

        let mut terminal = TerminalPane::with_settings(
            terminal_root_for_workspace(&workspace.root),
            settings.terminal_scrollback_rows,
            settings.terminal_shell_path.clone(),
            settings.terminal_shell_args.clone(),
            settings.terminal_cwd.clone(),
            settings.terminal_split_cwd,
            settings.terminal_min_rows,
            settings.terminal_min_columns,
            settings.terminal_font_size,
            settings.terminal_line_height,
            settings.terminal_letter_spacing,
            settings.terminal_cursor_style,
            settings.terminal_cursor_width,
            settings.terminal_cursor_blinking,
            settings.terminal_cursor_style_inactive,
            settings.terminal_draw_bold_text_in_bright_colors,
            settings.terminal_minimum_contrast_ratio,
            settings.terminal_enable_bell,
            settings.terminal_bell_duration_ms,
            settings.terminal_show_exit_alert,
            settings.terminal_hide_on_last_closed,
            settings.terminal_confirm_on_kill,
            settings.terminal_tabs_enabled,
            settings.terminal_tabs_default_icon.clone(),
            settings.terminal_tabs_default_color.clone(),
            settings.terminal_tabs_allow_agent_cli_title,
            settings.terminal_tabs_title.clone(),
            settings.terminal_tabs_hide_condition,
            settings.terminal_tabs_show_active_terminal,
            settings.terminal_tabs_show_actions,
            settings.terminal_tabs_focus_mode,
            settings.terminal_tabs_location,
            settings.terminal_right_click_behavior,
            settings.terminal_middle_click_behavior,
            settings.terminal_alt_click_moves_cursor,
            settings.terminal_copy_on_selection,
            settings.terminal_ignore_bracketed_paste_mode,
            settings.terminal_enable_multi_line_paste_warning,
            settings.terminal_word_separators.clone(),
            settings.terminal_mouse_wheel_scroll_sensitivity,
            settings.terminal_fast_scroll_sensitivity,
            settings.terminal_mouse_wheel_zoom,
        );
        terminal.set_repaint_context(cc.egui_ctx.clone());
        startup_profiler.record("Create terminal");

        let watcher = None;
        startup_profiler.record("Start watcher");

        let now = Instant::now();
        let startup_timings = startup_profiler.into_entries();

        Ok(Self {
            runtime,
            tx,
            rx,
            workspace,
            settings,
            settings_panel_draft,
            settings_editor_font_path,
            settings_ui_font_path,
            theme_picker_selected,
            saved_session,
            terminal,
            watcher,
            recent_projects: app_state.recent_projects,
            trusted_workspaces: app_state.trusted_workspaces,
            now,
            startup_timings,
        })
    }
}

fn apply_restricted_app_state_vim_settings(settings: &mut EditorSettings, app_state: &AppState) {
    if let Some(vim_keybindings) = app_state.vim_keybindings {
        settings.vim_keybindings = vim_keybindings;
    }
    if let Some(vim) = &app_state.vim {
        settings.vim = vim.clone();
        sanitize_vim_settings_for_runtime(&mut settings.vim);
    }
}

fn create_runtime() -> anyhow::Result<Runtime> {
    Runtime::new().context("create tokio runtime")
}

pub(crate) fn empty_startup_workspace_root() -> PathBuf {
    crate::persistence_storage::app_state_dir().join(EMPTY_STARTUP_WORKSPACE_DIR_NAME)
}

pub(crate) fn terminal_root_for_workspace(workspace_root: &Path) -> PathBuf {
    terminal_root_for_workspace_with_home(workspace_root, home_dir_from_env())
}

fn terminal_root_for_workspace_with_home(workspace_root: &Path, home: Option<PathBuf>) -> PathBuf {
    if is_empty_startup_workspace_root(workspace_root) {
        home.unwrap_or_else(empty_startup_workspace_root)
    } else {
        workspace_root.to_path_buf()
    }
}

fn home_dir_from_env() -> Option<PathBuf> {
    home_dir_from_env_values(std::env::var_os("USERPROFILE"), std::env::var_os("HOME"))
}

fn home_dir_from_env_values(
    userprofile: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    non_empty_path(userprofile).or_else(|| non_empty_path(home))
}

fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value?);
    (!path.as_os_str().is_empty()).then_some(path)
}

pub(crate) fn is_empty_startup_workspace_root(path: &Path) -> bool {
    paths_match_lexically(path, &empty_startup_workspace_root())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_restricted_app_state_vim_settings, create_runtime, empty_startup_workspace_root,
        home_dir_from_env_values, is_empty_startup_workspace_root,
        terminal_root_for_workspace_with_home,
    };
    use crate::persistence::AppState;
    use kuroya_core::{EditorSettings, EditorVimKeyOverride, EditorVimSettings};
    use std::{ffi::OsString, path::PathBuf};

    #[test]
    fn startup_runtime_creation_returns_result() -> anyhow::Result<()> {
        let _runtime = create_runtime()?;
        Ok(())
    }

    #[test]
    fn empty_startup_workspace_root_is_recognized() {
        let root = empty_startup_workspace_root();

        assert!(is_empty_startup_workspace_root(&root));
        assert!(!is_empty_startup_workspace_root(&root.join("child")));
    }

    #[cfg(windows)]
    #[test]
    fn empty_startup_workspace_root_recognizes_windows_verbatim_path() {
        let root = empty_startup_workspace_root();
        let verbatim = PathBuf::from(format!(r"\\?\{}", root.display()));

        assert!(is_empty_startup_workspace_root(&verbatim));
    }

    #[test]
    fn empty_startup_terminal_root_prefers_user_home() {
        assert_eq!(
            home_dir_from_env_values(
                Some(OsString::from(r"C:\Users\kuroya")),
                Some(OsString::from("/home/kuroya"))
            ),
            Some(PathBuf::from(r"C:\Users\kuroya"))
        );
        assert_eq!(
            home_dir_from_env_values(Some(OsString::new()), Some(OsString::from("/home/kuroya"))),
            Some(PathBuf::from("/home/kuroya"))
        );
    }

    #[test]
    fn terminal_root_for_empty_workspace_uses_home() {
        let root = empty_startup_workspace_root();

        assert_eq!(
            terminal_root_for_workspace_with_home(&root, Some(PathBuf::from(r"C:\Users\kuroya"))),
            PathBuf::from(r"C:\Users\kuroya")
        );
        assert_eq!(
            terminal_root_for_workspace_with_home(PathBuf::from("project").as_path(), None),
            PathBuf::from("project")
        );
    }

    #[test]
    fn restricted_startup_restores_vim_keybindings_and_config_from_app_state() {
        let mut settings = EditorSettings::default();
        let app_state_vim = EditorVimSettings {
            disabled_bindings: vec!["Q".to_owned(), "<Nope>".to_owned()],
            key_overrides: vec![
                EditorVimKeyOverride {
                    before: "<Home>".to_owned(),
                    after: "0".to_owned(),
                    command: None,
                },
                EditorVimKeyOverride {
                    before: "L".to_owned(),
                    after: "<Left>".to_owned(),
                    command: None,
                },
            ],
        };
        let app_state = AppState {
            vim_keybindings: Some(true),
            vim: Some(app_state_vim),
            ..AppState::default()
        };

        apply_restricted_app_state_vim_settings(&mut settings, &app_state);

        assert!(settings.vim_keybindings);
        assert_eq!(
            settings.vim,
            EditorVimSettings {
                disabled_bindings: vec!["Q".to_owned()],
                key_overrides: vec![EditorVimKeyOverride {
                    before: "<Home>".to_owned(),
                    after: "0".to_owned(),
                    command: None,
                }],
            }
        );
    }
}
