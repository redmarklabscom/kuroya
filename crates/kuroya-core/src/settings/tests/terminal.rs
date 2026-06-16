use super::*;

#[test]
fn terminal_scrollback_rows_are_clamped_to_reasonable_range() {
    assert_eq!(
        clamp_terminal_scrollback_rows(1),
        MIN_TERMINAL_SCROLLBACK_ROWS
    );
    assert_eq!(clamp_terminal_scrollback_rows(10_000), 10_000);
    assert_eq!(
        clamp_terminal_scrollback_rows(usize::MAX),
        MAX_TERMINAL_SCROLLBACK_ROWS
    );
}

#[test]
fn terminal_minimum_size_is_clamped_to_reasonable_range() {
    assert_eq!(clamp_terminal_min_rows(0), MIN_TERMINAL_MIN_ROWS);
    assert_eq!(clamp_terminal_min_rows(24), 24);
    assert_eq!(clamp_terminal_min_rows(u16::MAX), MAX_TERMINAL_MIN_ROWS);
    assert_eq!(clamp_terminal_min_columns(1), MIN_TERMINAL_MIN_COLUMNS);
    assert_eq!(clamp_terminal_min_columns(100), 100);
    assert_eq!(
        clamp_terminal_min_columns(u16::MAX),
        MAX_TERMINAL_MIN_COLUMNS
    );
}

#[test]
fn terminal_font_metrics_are_clamped_to_reasonable_ranges() {
    assert_eq!(clamp_terminal_font_size(1.0), MIN_TERMINAL_FONT_SIZE);
    assert_eq!(clamp_terminal_font_size(14.0), 14.0);
    assert_eq!(
        clamp_terminal_font_size(f32::INFINITY),
        DEFAULT_TERMINAL_FONT_SIZE
    );
    assert_eq!(clamp_terminal_line_height(0.1), MIN_TERMINAL_LINE_HEIGHT);
    assert_eq!(clamp_terminal_line_height(1.4), 1.4);
    assert_eq!(
        clamp_terminal_line_height(f32::NAN),
        DEFAULT_TERMINAL_LINE_HEIGHT
    );
}

#[test]
fn terminal_cursor_settings_parse_vs_code_style_values() {
    let settings: EditorSettings = toml::from_str(
        "terminal_cursor_style = \"line\"\n\
             terminal_cursor_style_inactive = \"none\"\n\
             terminal_cursor_width = 3.0\n\
             terminal_cursor_blinking = true\n\
             terminal_shell_path = \"pwsh.exe\"\n\
             terminal_shell_args = [\"-NoLogo\", \"-NoProfile\"]\n\
             terminal_cwd = \"tools\"\n\
             terminal_split_cwd = \"workspaceRoot\"\n\
             terminal_min_rows = 8\n\
             terminal_min_columns = 40\n\
             terminal_letter_spacing = 2.0\n\
             terminal_minimum_contrast_ratio = 7.0\n\
             terminal_enable_bell = true\n\
             terminal_bell_duration_ms = 750\n\
             terminal_show_exit_alert = false\n\
             terminal_hide_on_startup = \"always\"\n\
             terminal_hide_on_last_closed = false\n\
             terminal_confirm_on_exit = \"always\"\n\
             terminal_confirm_on_kill = \"panel\"\n\
             terminal_tabs_enabled = false\n\
             terminal_tabs_default_icon = \"code\"\n\
             terminal_tabs_default_color = \"terminal.ansiBlue\"\n\
             terminal_tabs_allow_agent_cli_title = false\n\
             terminal_tabs_title = \"${process} - ${cwd}\"\n\
             terminal_tabs_hide_condition = \"never\"\n\
             terminal_tabs_show_active_terminal = \"always\"\n\
             terminal_tabs_show_actions = \"always\"\n\
             terminal_tabs_focus_mode = \"singleClick\"\n\
             terminal_tabs_location = \"left\"\n\
             terminal_right_click_behavior = \"copyPaste\"\n\
             terminal_middle_click_behavior = \"paste\"\n\
             terminal_alt_click_moves_cursor = false\n\
             terminal_copy_on_selection = true\n\
             terminal_ignore_bracketed_paste_mode = true\n\
             terminal_enable_multi_line_paste_warning = \"always\"\n\
             terminal_word_separators = \":\"\n\
             terminal_mouse_wheel_scroll_sensitivity = 2.0\n\
             terminal_fast_scroll_sensitivity = 8.0\n\
             terminal_mouse_wheel_zoom = true\n",
    )
    .expect("terminal cursor settings should load");

    assert_eq!(settings.terminal_cursor_style, TerminalCursorStyle::Line);
    assert_eq!(
        settings.terminal_cursor_style_inactive,
        TerminalInactiveCursorStyle::None
    );
    assert_eq!(settings.terminal_cursor_width, 3.0);
    assert!(settings.terminal_cursor_blinking);
    assert_eq!(settings.terminal_shell_path.as_deref(), Some("pwsh.exe"));
    assert_eq!(settings.terminal_shell_args, ["-NoLogo", "-NoProfile"]);
    assert_eq!(settings.terminal_cwd.as_deref(), Some("tools"));
    assert_eq!(settings.terminal_split_cwd, TerminalSplitCwd::WorkspaceRoot);
    assert_eq!(settings.terminal_min_rows, 8);
    assert_eq!(settings.terminal_min_columns, 40);
    assert_eq!(settings.terminal_letter_spacing, 2.0);
    assert_eq!(settings.terminal_minimum_contrast_ratio, 7.0);
    assert!(settings.terminal_enable_bell);
    assert_eq!(settings.terminal_bell_duration_ms, 750);
    assert!(!settings.terminal_show_exit_alert);
    assert_eq!(
        settings.terminal_hide_on_startup,
        TerminalHideOnStartup::Always
    );
    assert!(!settings.terminal_hide_on_last_closed);
    assert_eq!(
        settings.terminal_confirm_on_exit,
        TerminalConfirmOnExit::Always
    );
    assert_eq!(
        settings.terminal_confirm_on_kill,
        TerminalConfirmOnKill::Panel
    );
    assert!(!settings.terminal_tabs_enabled);
    assert_eq!(settings.terminal_tabs_default_icon, "code");
    assert_eq!(
        settings.terminal_tabs_default_color.as_deref(),
        Some("terminal.ansiBlue")
    );
    assert!(!settings.terminal_tabs_allow_agent_cli_title);
    assert_eq!(settings.terminal_tabs_title, "${process} - ${cwd}");
    assert_eq!(
        settings.terminal_tabs_hide_condition,
        TerminalTabsHideCondition::Never
    );
    assert_eq!(
        settings.terminal_tabs_show_active_terminal,
        TerminalTabsShowActiveTerminal::Always
    );
    assert_eq!(
        settings.terminal_tabs_show_actions,
        TerminalTabsShowActions::Always
    );
    assert_eq!(
        settings.terminal_tabs_focus_mode,
        TerminalTabsFocusMode::SingleClick
    );
    assert_eq!(settings.terminal_tabs_location, TerminalTabsLocation::Left);
    assert_eq!(
        settings.terminal_right_click_behavior,
        TerminalRightClickBehavior::CopyPaste
    );
    assert_eq!(
        settings.terminal_middle_click_behavior,
        TerminalMiddleClickBehavior::Paste
    );
    assert!(!settings.terminal_alt_click_moves_cursor);
    assert!(settings.terminal_copy_on_selection);
    assert!(settings.terminal_ignore_bracketed_paste_mode);
    assert_eq!(
        settings.terminal_enable_multi_line_paste_warning,
        TerminalMultiLinePasteWarning::Always
    );
    assert_eq!(settings.terminal_word_separators, ":");
    assert_eq!(settings.terminal_mouse_wheel_scroll_sensitivity, 2.0);
    assert_eq!(settings.terminal_fast_scroll_sensitivity, 8.0);
    assert!(settings.terminal_mouse_wheel_zoom);
}

#[test]
fn terminal_cursor_width_is_clamped_to_reasonable_range() {
    assert_eq!(clamp_terminal_cursor_width(0.0), MIN_TERMINAL_CURSOR_WIDTH);
    assert_eq!(clamp_terminal_cursor_width(3.0), 3.0);
    assert_eq!(
        clamp_terminal_cursor_width(f32::INFINITY),
        DEFAULT_TERMINAL_CURSOR_WIDTH
    );
}

#[test]
fn terminal_spacing_contrast_and_bell_settings_are_clamped() {
    assert_eq!(
        clamp_terminal_letter_spacing(-1.0),
        MIN_TERMINAL_LETTER_SPACING
    );
    assert_eq!(clamp_terminal_letter_spacing(2.5), 2.5);
    assert_eq!(
        clamp_terminal_letter_spacing(f32::NAN),
        DEFAULT_TERMINAL_LETTER_SPACING
    );
    assert_eq!(
        clamp_terminal_minimum_contrast_ratio(0.0),
        MIN_TERMINAL_MINIMUM_CONTRAST_RATIO
    );
    assert_eq!(clamp_terminal_minimum_contrast_ratio(4.5), 4.5);
    assert_eq!(
        clamp_terminal_minimum_contrast_ratio(f32::INFINITY),
        DEFAULT_TERMINAL_MINIMUM_CONTRAST_RATIO
    );
    assert_eq!(
        clamp_terminal_bell_duration_ms(1),
        MIN_TERMINAL_BELL_DURATION_MS
    );
    assert_eq!(clamp_terminal_bell_duration_ms(750), 750);
    assert_eq!(
        clamp_terminal_bell_duration_ms(u64::MAX),
        MAX_TERMINAL_BELL_DURATION_MS
    );
}

#[test]
fn terminal_scroll_sensitivity_is_clamped_to_reasonable_range() {
    assert_eq!(
        clamp_terminal_scroll_sensitivity(-1.0, DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY),
        MIN_TERMINAL_SCROLL_SENSITIVITY
    );
    assert_eq!(
        clamp_terminal_scroll_sensitivity(5.0, DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY),
        5.0
    );
    assert_eq!(
        clamp_terminal_scroll_sensitivity(f32::INFINITY, DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY),
        DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY
    );
    assert_eq!(
        clamp_terminal_scroll_sensitivity(f32::MAX, DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY),
        MAX_TERMINAL_SCROLL_SENSITIVITY
    );
}
