use crate::{
    KuroyaApp,
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, display_path_label_cow,
        sanitized_display_label_cow,
    },
    settings_form::optional_setting_path_to_input,
    theme::selected_theme_index_with_plugins,
    workspace_state::settings_path,
};
use kuroya_core::{EditorSettings, EditorSettingsLoad};
use std::{borrow::Cow, fmt::Display, path::Path};

pub(crate) fn load_workspace_settings(
    root: &Path,
    workspace_trusted: bool,
) -> anyhow::Result<EditorSettingsLoad> {
    if !workspace_trusted {
        return Ok(EditorSettingsLoad {
            settings: EditorSettings::default(),
            quarantined_path: None,
        });
    }

    EditorSettings::load_or_create_with_recovery(&settings_path(root))
}

impl KuroyaApp {
    pub(crate) fn reload_settings(&mut self) {
        let path = settings_path(&self.workspace.root);
        match load_workspace_settings(&self.workspace.root, self.workspace_trusted) {
            Ok(loaded) => {
                let workspace_trusted = self.workspace_trusted;
                let mut settings = loaded.settings;
                settings.vim_keybindings = self.settings.vim_keybindings;
                let previous_settings = std::mem::replace(&mut self.settings, settings);
                for buffer in &mut self.buffers {
                    buffer.set_word_separators(self.settings.word_separators.clone());
                }
                self.terminal
                    .set_scrollback_rows(self.settings.terminal_scrollback_rows);
                self.terminal.set_shell_profile(
                    self.settings.terminal_shell_path.clone(),
                    self.settings.terminal_shell_args.clone(),
                );
                self.terminal
                    .set_terminal_cwd(self.settings.terminal_cwd.clone());
                self.terminal
                    .set_split_cwd(self.settings.terminal_split_cwd);
                self.terminal.set_minimum_size(
                    self.settings.terminal_min_rows,
                    self.settings.terminal_min_columns,
                );
                self.terminal.set_font_metrics(
                    self.settings.terminal_font_size,
                    self.settings.terminal_line_height,
                    self.settings.terminal_letter_spacing,
                );
                self.terminal.set_cursor_settings(
                    self.settings.terminal_cursor_style,
                    self.settings.terminal_cursor_width,
                    self.settings.terminal_cursor_blinking,
                    self.settings.terminal_cursor_style_inactive,
                );
                self.terminal.set_draw_bold_text_in_bright_colors(
                    self.settings.terminal_draw_bold_text_in_bright_colors,
                );
                self.terminal
                    .set_minimum_contrast_ratio(self.settings.terminal_minimum_contrast_ratio);
                self.terminal.set_bell_settings(
                    self.settings.terminal_enable_bell,
                    self.settings.terminal_bell_duration_ms,
                );
                self.terminal
                    .set_show_exit_alert(self.settings.terminal_show_exit_alert);
                self.terminal
                    .set_hide_on_last_closed(self.settings.terminal_hide_on_last_closed);
                self.terminal
                    .set_confirm_on_kill(self.settings.terminal_confirm_on_kill);
                self.terminal
                    .set_tabs_enabled(self.settings.terminal_tabs_enabled);
                self.terminal
                    .set_tabs_default_icon(&self.settings.terminal_tabs_default_icon);
                self.terminal
                    .set_tabs_default_color(self.settings.terminal_tabs_default_color.clone());
                self.terminal.set_tabs_allow_agent_cli_title(
                    self.settings.terminal_tabs_allow_agent_cli_title,
                );
                self.terminal
                    .set_tabs_title_template(&self.settings.terminal_tabs_title);
                self.terminal
                    .set_tabs_hide_condition(self.settings.terminal_tabs_hide_condition);
                self.terminal.set_tabs_show_active_terminal(
                    self.settings.terminal_tabs_show_active_terminal,
                );
                self.terminal
                    .set_tabs_show_actions(self.settings.terminal_tabs_show_actions);
                self.terminal
                    .set_tabs_focus_mode(self.settings.terminal_tabs_focus_mode);
                self.terminal
                    .set_tabs_location(self.settings.terminal_tabs_location);
                self.terminal
                    .set_right_click_behavior(self.settings.terminal_right_click_behavior);
                self.terminal
                    .set_middle_click_behavior(self.settings.terminal_middle_click_behavior);
                self.terminal
                    .set_alt_click_moves_cursor(self.settings.terminal_alt_click_moves_cursor);
                self.terminal
                    .set_copy_on_selection(self.settings.terminal_copy_on_selection);
                self.terminal.set_ignore_bracketed_paste_mode(
                    self.settings.terminal_ignore_bracketed_paste_mode,
                );
                self.terminal.set_multi_line_paste_warning(
                    self.settings.terminal_enable_multi_line_paste_warning,
                );
                self.terminal
                    .set_word_separators(self.settings.terminal_word_separators.clone());
                self.terminal.set_scroll_sensitivity(
                    self.settings.terminal_mouse_wheel_scroll_sensitivity,
                    self.settings.terminal_fast_scroll_sensitivity,
                );
                self.terminal
                    .set_mouse_wheel_zoom(self.settings.terminal_mouse_wheel_zoom);
                self.sync_settings_panel_inputs();
                self.theme_picker_selected =
                    selected_theme_index_with_plugins(&self.settings.theme, &self.plugin_themes);
                self.status = if workspace_trusted {
                    settings_reload_status(&path, loaded.quarantined_path.as_deref())
                } else {
                    restricted_workspace_settings_status()
                };
                self.theme_dirty = true;
                self.fonts_dirty = true;
                if previous_settings.read_only != self.settings.read_only {
                    self.sync_global_read_only_buffers();
                }
                let git_repository_scan_settings_changed = previous_settings
                    .git_auto_repository_detection
                    != self.settings.git_auto_repository_detection
                    || previous_settings.git_ignore_submodules
                        != self.settings.git_ignore_submodules
                    || previous_settings.git_repository_scan_ignored_folders
                        != self.settings.git_repository_scan_ignored_folders
                    || previous_settings.git_open_repository_in_parent_folders
                        != self.settings.git_open_repository_in_parent_folders
                    || previous_settings.git_detect_submodules
                        != self.settings.git_detect_submodules
                    || previous_settings.git_detect_submodules_limit
                        != self.settings.git_detect_submodules_limit
                    || previous_settings.git_repository_scan_max_depth
                        != self.settings.git_repository_scan_max_depth
                    || previous_settings.git_detect_worktrees != self.settings.git_detect_worktrees
                    || previous_settings.git_detect_worktrees_limit
                        != self.settings.git_detect_worktrees_limit
                    || previous_settings.git_scan_repositories
                        != self.settings.git_scan_repositories
                    || previous_settings.git_worktree_include_files
                        != self.settings.git_worktree_include_files
                    || previous_settings.git_similarity_threshold
                        != self.settings.git_similarity_threshold;
                let git_ignored_repositories_changed = previous_settings.git_ignored_repositories
                    != self.settings.git_ignored_repositories;
                if previous_settings.diff_max_file_size_mb != self.settings.diff_max_file_size_mb
                    || previous_settings.git_enabled != self.settings.git_enabled
                    || git_ignored_repositories_changed
                    || git_repository_scan_settings_changed
                {
                    self.invalidate_virtual_source_control_open_requests();
                }
                if previous_settings.git_blame_ignore_whitespace
                    != self.settings.git_blame_ignore_whitespace
                {
                    self.sync_source_control_blame_settings();
                }
                if previous_settings.git_enabled != self.settings.git_enabled {
                    self.sync_git_enabled_state();
                } else if previous_settings.git_autorefresh != self.settings.git_autorefresh {
                    self.sync_git_autorefresh_state(previous_settings.git_autorefresh);
                } else if git_ignored_repositories_changed {
                    self.sync_git_repository_filters_state();
                } else if git_repository_scan_settings_changed {
                    self.spawn_git_scan();
                }
            }
            Err(error) => {
                self.status = settings_load_failed_status(error);
            }
        }
    }

    pub(crate) fn sync_settings_panel_inputs(&mut self) {
        self.settings_panel_draft = self.settings.clone();
        self.settings_editor_font_path =
            optional_setting_path_to_input(&self.settings.editor_font_path);
        self.settings_ui_font_path = optional_setting_path_to_input(&self.settings.ui_font_path);
    }

    pub(crate) fn open_settings_file(&mut self) {
        let path = settings_path(&self.workspace.root);
        if !path.exists() {
            if let Err(error) = self.settings.save(&path) {
                self.status = settings_create_failed_status(error);
                return;
            }
        }
        self.spawn_open_file(path.clone());
        self.status = settings_opening_status(&path);
    }
}

fn restricted_workspace_settings_status() -> String {
    "Restricted workspace: using default settings".to_owned()
}

pub(crate) fn settings_reload_status(path: &Path, quarantined_path: Option<&Path>) -> String {
    let path = settings_path_status_label(path);
    match quarantined_path {
        Some(quarantined_path) => {
            let quarantined_path = display_path_label_cow(quarantined_path);
            format!(
                "Recovered settings from {}; corrupt file moved to {}",
                path.as_ref(),
                quarantined_path.as_ref()
            )
        }
        None => format!("Loaded settings from {}", path.as_ref()),
    }
}

fn settings_load_failed_status(error: impl Display) -> String {
    let error = error.to_string();
    let error = settings_error_status_label(&error);
    format!("Could not load settings: {}", error.as_ref())
}

fn settings_create_failed_status(error: impl Display) -> String {
    let error = error.to_string();
    let error = settings_error_status_label(&error);
    format!("Could not create settings file: {}", error.as_ref())
}

fn settings_opening_status(path: &Path) -> String {
    let path = display_path_label_cow(path);
    format!("Opening settings {}", path.as_ref())
}

fn settings_path_status_label(path: &Path) -> Cow<'_, str> {
    if let Some(path) = path.to_str() {
        return sanitized_display_label_cow(path, DISPLAY_PATH_LABEL_MAX_CHARS, ".");
    }

    Cow::Owned(settings_path_owned_display_label(
        path.display().to_string(),
    ))
}

fn settings_path_owned_display_label(path: String) -> String {
    match sanitized_display_label_cow(&path, DISPLAY_PATH_LABEL_MAX_CHARS, ".") {
        Cow::Borrowed(label) if label.as_ptr() == path.as_ptr() && label.len() == path.len() => {
            path
        }
        Cow::Borrowed(label) => label.to_owned(),
        Cow::Owned(label) => label,
    }
}

fn settings_error_status_label(error: &str) -> Cow<'_, str> {
    display_error_label_cow(error)
}

#[cfg(test)]
mod tests {
    use super::{
        KuroyaApp, load_workspace_settings, restricted_workspace_settings_status,
        settings_create_failed_status, settings_load_failed_status, settings_opening_status,
        settings_path_status_label, settings_reload_status,
    };
    use crate::{
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
        workspace_state::settings_path,
    };
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{
        borrow::Cow,
        fs,
        path::{Path, PathBuf},
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn settings_reload_status_reports_recovered_corrupt_file() {
        assert_eq!(
            settings_reload_status(
                Path::new("workspace/.kuroya/settings.toml"),
                Some(Path::new("workspace/.kuroya/settings.toml.corrupt.42"))
            ),
            "Recovered settings from workspace/.kuroya/settings.toml; corrupt file moved to settings.toml.corrupt.42"
        );
    }

    #[test]
    fn settings_reload_status_sanitizes_and_bounds_paths() {
        let path = Path::new("workspace/.kuroya")
            .join(format!("bad\n{}\u{202e}.toml", "segment-".repeat(40)));
        let quarantined_path = Path::new("workspace/.kuroya")
            .join(format!("corrupt\n{}\u{202e}.toml", "tail-".repeat(40)));

        let status = settings_reload_status(&path, Some(&quarantined_path));

        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Recovered settings from ; corrupt file moved to "
                    .chars()
                    .count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn settings_path_status_label_borrows_clean_ascii_and_unicode_paths() {
        for path in [
            Path::new("workspace/.kuroya/settings.toml"),
            Path::new("workspace/.kuroya/settings-\u{03bb}.toml"),
        ] {
            let expected = path.to_str().expect("test path is UTF-8");

            match settings_path_status_label(path) {
                Cow::Borrowed(label) => assert_eq!(label, expected),
                Cow::Owned(label) => panic!("expected borrowed settings path, got {label:?}"),
            }
        }
    }

    #[test]
    fn settings_path_status_label_owns_dirty_truncated_and_fallback_paths() {
        let long = Path::new("workspace/.kuroya").join(format!(
            "{}settings.toml",
            "very-long-segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        for path in [
            Path::new("workspace/.kuroya/bad\nsettings\u{202e}.toml"),
            long.as_path(),
            Path::new("\n\t\u{202e}"),
        ] {
            let label = settings_path_status_label(path);

            assert!(matches!(&label, Cow::Owned(_)));
            assert!(!label.contains('\n'));
            assert!(!label.contains('\u{202e}'));
            assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        }
        assert_eq!(
            settings_path_status_label(Path::new("\n\t\u{202e}")).as_ref(),
            "."
        );
    }

    #[test]
    fn settings_error_statuses_sanitize_and_bound_errors() {
        let error = format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        );

        let load_status = settings_load_failed_status(&error);
        let create_status = settings_create_failed_status(&error);

        for status in [load_status, create_status] {
            assert!(!status.contains('\n'));
            assert!(!status.contains('\u{202e}'));
            assert!(status.contains("..."));
            assert!(
                status.chars().count()
                    <= "Could not create settings file: ".chars().count()
                        + DISPLAY_ERROR_LABEL_MAX_CHARS
            );
        }
    }

    #[test]
    fn settings_opening_status_sanitizes_and_bounds_path() {
        let path = Path::new("workspace/.kuroya")
            .join(format!("bad\n{}\u{202e}.toml", "segment-".repeat(40)));

        let status = settings_opening_status(&path);

        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Opening settings ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn reload_settings_updates_open_buffer_word_separators() {
        let root = temp_root("word-separators");
        let settings = settings_path(&root);
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(&settings, "word_separators = \".\"\n").unwrap();

        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(root.join("src/main.rs")),
            "alpha.beta".to_owned(),
        ));

        app.reload_settings();

        assert_eq!(app.settings.word_separators, ".");
        assert_eq!(app.buffer(1).unwrap().word_separators(), ".");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reload_settings_preserves_app_vim_keybindings() {
        let root = temp_root("preserve-vim-keybindings");
        let settings = settings_path(&root);
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(
            &settings,
            "vim_keybindings = false\nword_separators = \".\"\n",
        )
        .unwrap();

        let mut app = app_for_test(root.clone());
        app.settings.vim_keybindings = true;

        app.reload_settings();

        assert!(app.settings.vim_keybindings);
        assert_eq!(app.settings.word_separators, ".");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_workspace_settings_uses_default_for_untrusted_workspace() {
        let root = temp_root("untrusted-load-defaults");
        let settings = settings_path(&root);
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(&settings, "word_separators = \".\"\n").unwrap();

        let loaded = load_workspace_settings(&root, false).unwrap();

        assert_eq!(loaded.settings, EditorSettings::default());
        assert_eq!(loaded.quarantined_path, None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reload_settings_ignores_untrusted_workspace_settings() {
        let root = temp_root("untrusted-reload-defaults");
        let settings = settings_path(&root);
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(&settings, "word_separators = \".\"\n").unwrap();

        let mut app = app_for_test_with_trust(root.clone(), false);
        app.settings.word_separators = ".".to_owned();

        app.reload_settings();

        assert_eq!(
            app.settings.word_separators,
            EditorSettings::default().word_separators
        );
        assert_eq!(app.status, restricted_workspace_settings_status());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn open_settings_file_creates_missing_settings_and_opens_it() {
        let root = temp_root("open-settings-missing");
        let settings_path = settings_path(&root);
        let mut app = app_for_test(root.clone());
        app.settings.word_separators = ".".to_owned();

        app.open_settings_file();

        assert!(settings_path.is_file());
        assert_eq!(app.status, settings_opening_status(&settings_path));
        assert!(app.pending_open_paths.contains(&settings_path));
        assert_eq!(
            EditorSettings::load_or_create(&settings_path)
                .unwrap()
                .word_separators,
            "."
        );

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn open_settings_file_preserves_existing_settings_and_opens_it() {
        let root = temp_root("open-settings-existing");
        let settings_path = settings_path(&root);
        let existing_settings = "word_separators = \":\"\n";
        fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        fs::write(&settings_path, existing_settings).unwrap();
        let mut app = app_for_test(root.clone());
        app.settings.word_separators = ".".to_owned();

        app.open_settings_file();

        assert_eq!(
            fs::read_to_string(&settings_path).unwrap(),
            existing_settings
        );
        assert_eq!(app.status, settings_opening_status(&settings_path));
        assert!(app.pending_open_paths.contains(&settings_path));

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        app_for_test_with_trust(root, true)
    }

    fn app_for_test_with_trust(root: PathBuf, trusted: bool) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        let trusted_workspaces = trusted.then(|| root.clone()).into_iter().collect();
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
            trusted_workspaces,
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
            "kuroya-settings-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
