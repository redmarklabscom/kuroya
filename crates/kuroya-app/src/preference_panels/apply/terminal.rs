use crate::terminal::TerminalPane;
use kuroya_core::EditorSettings;

pub(super) fn sync_terminal_settings(terminal: &mut TerminalPane, settings: &EditorSettings) {
    terminal.set_scrollback_rows(settings.terminal_scrollback_rows);
    terminal.set_shell_profile(
        settings.terminal_shell_path.clone(),
        settings.terminal_shell_args.clone(),
    );
    terminal.set_terminal_cwd(settings.terminal_cwd.clone());
    terminal.set_split_cwd(settings.terminal_split_cwd);
    terminal.set_minimum_size(settings.terminal_min_rows, settings.terminal_min_columns);
    terminal.set_font_metrics(
        settings.terminal_font_size,
        settings.terminal_line_height,
        settings.terminal_letter_spacing,
    );
    terminal.set_cursor_settings(
        settings.terminal_cursor_style,
        settings.terminal_cursor_width,
        settings.terminal_cursor_blinking,
        settings.terminal_cursor_style_inactive,
    );
    terminal.set_draw_bold_text_in_bright_colors(settings.terminal_draw_bold_text_in_bright_colors);
    terminal.set_minimum_contrast_ratio(settings.terminal_minimum_contrast_ratio);
    terminal.set_bell_settings(
        settings.terminal_enable_bell,
        settings.terminal_bell_duration_ms,
    );
    terminal.set_show_exit_alert(settings.terminal_show_exit_alert);
    terminal.set_hide_on_last_closed(settings.terminal_hide_on_last_closed);
    terminal.set_confirm_on_kill(settings.terminal_confirm_on_kill);
    terminal.set_tabs_enabled(settings.terminal_tabs_enabled);
    terminal.set_tabs_default_icon(&settings.terminal_tabs_default_icon);
    terminal.set_tabs_default_color(settings.terminal_tabs_default_color.clone());
    terminal.set_tabs_allow_agent_cli_title(settings.terminal_tabs_allow_agent_cli_title);
    terminal.set_tabs_title_template(&settings.terminal_tabs_title);
    terminal.set_tabs_hide_condition(settings.terminal_tabs_hide_condition);
    terminal.set_tabs_show_active_terminal(settings.terminal_tabs_show_active_terminal);
    terminal.set_tabs_show_actions(settings.terminal_tabs_show_actions);
    terminal.set_tabs_focus_mode(settings.terminal_tabs_focus_mode);
    terminal.set_tabs_location(settings.terminal_tabs_location);
    terminal.set_right_click_behavior(settings.terminal_right_click_behavior);
    terminal.set_middle_click_behavior(settings.terminal_middle_click_behavior);
    terminal.set_alt_click_moves_cursor(settings.terminal_alt_click_moves_cursor);
    terminal.set_copy_on_selection(settings.terminal_copy_on_selection);
    terminal.set_ignore_bracketed_paste_mode(settings.terminal_ignore_bracketed_paste_mode);
    terminal.set_multi_line_paste_warning(settings.terminal_enable_multi_line_paste_warning);
    terminal.set_word_separators(settings.terminal_word_separators.clone());
    terminal.set_scroll_sensitivity(
        settings.terminal_mouse_wheel_scroll_sensitivity,
        settings.terminal_fast_scroll_sensitivity,
    );
    terminal.set_mouse_wheel_zoom(settings.terminal_mouse_wheel_zoom);
}
