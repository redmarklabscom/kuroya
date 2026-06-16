use kuroya_core::{
    DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY, DEFAULT_TERMINAL_MOUSE_WHEEL_SCROLL_SENSITIVITY,
    DEFAULT_TERMINAL_TABS_DEFAULT_ICON, DEFAULT_TERMINAL_TABS_TITLE, EditorSettings,
    clamp_terminal_bell_duration_ms, clamp_terminal_cursor_width, clamp_terminal_font_size,
    clamp_terminal_letter_spacing, clamp_terminal_line_height, clamp_terminal_min_columns,
    clamp_terminal_min_rows, clamp_terminal_minimum_contrast_ratio,
    clamp_terminal_scroll_sensitivity, clamp_terminal_scrollback_rows,
};

const MAX_TERMINAL_SETTING_TEXT_CHARS: usize = 8_192;

pub(super) fn apply_terminal_settings_draft(settings: &mut EditorSettings, draft: &EditorSettings) {
    settings.terminal_scrollback_rows =
        clamp_terminal_scrollback_rows(draft.terminal_scrollback_rows);
    settings.terminal_shell_path = draft
        .terminal_shell_path
        .as_deref()
        .and_then(normalized_terminal_setting);
    settings.terminal_shell_args = normalized_terminal_shell_args(&draft.terminal_shell_args);
    settings.terminal_cwd = draft
        .terminal_cwd
        .as_deref()
        .and_then(normalized_terminal_setting);
    settings.terminal_split_cwd = draft.terminal_split_cwd;
    settings.terminal_min_rows = clamp_terminal_min_rows(draft.terminal_min_rows);
    settings.terminal_min_columns = clamp_terminal_min_columns(draft.terminal_min_columns);
    settings.terminal_font_size = clamp_terminal_font_size(draft.terminal_font_size);
    settings.terminal_line_height = clamp_terminal_line_height(draft.terminal_line_height);
    settings.terminal_letter_spacing = clamp_terminal_letter_spacing(draft.terminal_letter_spacing);
    settings.terminal_cursor_style = draft.terminal_cursor_style;
    settings.terminal_cursor_width = clamp_terminal_cursor_width(draft.terminal_cursor_width);
    settings.terminal_cursor_blinking = draft.terminal_cursor_blinking;
    settings.terminal_cursor_style_inactive = draft.terminal_cursor_style_inactive;
    settings.terminal_draw_bold_text_in_bright_colors =
        draft.terminal_draw_bold_text_in_bright_colors;
    settings.terminal_minimum_contrast_ratio =
        clamp_terminal_minimum_contrast_ratio(draft.terminal_minimum_contrast_ratio);
    settings.terminal_enable_bell = draft.terminal_enable_bell;
    settings.terminal_bell_duration_ms =
        clamp_terminal_bell_duration_ms(draft.terminal_bell_duration_ms);
    settings.terminal_show_exit_alert = draft.terminal_show_exit_alert;
    settings.terminal_hide_on_startup = draft.terminal_hide_on_startup;
    settings.terminal_hide_on_last_closed = draft.terminal_hide_on_last_closed;
    settings.terminal_confirm_on_exit = draft.terminal_confirm_on_exit;
    settings.terminal_confirm_on_kill = draft.terminal_confirm_on_kill;
    settings.terminal_tabs_enabled = draft.terminal_tabs_enabled;
    settings.terminal_tabs_default_icon = normalized_setting_text(
        &draft.terminal_tabs_default_icon,
        DEFAULT_TERMINAL_TABS_DEFAULT_ICON,
    );
    settings.terminal_tabs_default_color = draft
        .terminal_tabs_default_color
        .as_deref()
        .and_then(raw_optional_setting_text);
    settings.terminal_tabs_allow_agent_cli_title = draft.terminal_tabs_allow_agent_cli_title;
    settings.terminal_tabs_title =
        normalized_setting_text(&draft.terminal_tabs_title, DEFAULT_TERMINAL_TABS_TITLE);
    settings.terminal_tabs_hide_condition = draft.terminal_tabs_hide_condition;
    settings.terminal_tabs_show_active_terminal = draft.terminal_tabs_show_active_terminal;
    settings.terminal_tabs_show_actions = draft.terminal_tabs_show_actions;
    settings.terminal_tabs_focus_mode = draft.terminal_tabs_focus_mode;
    settings.terminal_tabs_location = draft.terminal_tabs_location;
    settings.terminal_right_click_behavior = draft.terminal_right_click_behavior;
    settings.terminal_middle_click_behavior = draft.terminal_middle_click_behavior;
    settings.terminal_alt_click_moves_cursor = draft.terminal_alt_click_moves_cursor;
    settings.terminal_copy_on_selection = draft.terminal_copy_on_selection;
    settings.terminal_ignore_bracketed_paste_mode = draft.terminal_ignore_bracketed_paste_mode;
    settings.terminal_enable_multi_line_paste_warning =
        draft.terminal_enable_multi_line_paste_warning;
    settings.terminal_word_separators =
        normalized_setting_text_value(&draft.terminal_word_separators);
    settings.terminal_mouse_wheel_scroll_sensitivity = clamp_terminal_scroll_sensitivity(
        draft.terminal_mouse_wheel_scroll_sensitivity,
        DEFAULT_TERMINAL_MOUSE_WHEEL_SCROLL_SENSITIVITY,
    );
    settings.terminal_fast_scroll_sensitivity = clamp_terminal_scroll_sensitivity(
        draft.terminal_fast_scroll_sensitivity,
        DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY,
    );
    settings.terminal_mouse_wheel_zoom = draft.terminal_mouse_wheel_zoom;
}

fn normalized_setting_text(value: &str, fallback: &str) -> String {
    normalized_non_empty_setting_text(value).unwrap_or_else(|| fallback.to_owned())
}

fn normalized_terminal_shell_args(args: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(args.len());
    for arg in args {
        if let Some(arg) = normalized_terminal_setting(arg) {
            normalized.push(arg);
        }
    }
    normalized
}

fn normalized_terminal_setting(value: &str) -> Option<String> {
    (!value.trim().is_empty() && !contains_terminal_setting_hidden_or_control(value))
        .then(|| value.to_owned())
}

fn contains_terminal_setting_hidden_or_control(value: &str) -> bool {
    value.chars().any(|ch| {
        ch.is_control()
            || matches!(ch, '\u{2028}' | '\u{2029}')
            || is_hidden_terminal_setting_format_control(ch)
    })
}

fn raw_optional_setting_text(value: &str) -> Option<String> {
    normalized_non_empty_setting_text(value)
}

fn normalized_non_empty_setting_text(value: &str) -> Option<String> {
    let normalized = normalized_setting_text_value(value);
    (!normalized.trim().is_empty()).then_some(normalized)
}

fn normalized_setting_text_value(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len().min(MAX_TERMINAL_SETTING_TEXT_CHARS));
    for ch in value.chars().take(MAX_TERMINAL_SETTING_TEXT_CHARS) {
        if is_hidden_terminal_setting_format_control(ch) {
            continue;
        }
        normalized.push(
            if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                ch
            },
        );
    }
    normalized
}

fn is_hidden_terminal_setting_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::{MAX_TERMINAL_SETTING_TEXT_CHARS, apply_terminal_settings_draft};
    use kuroya_core::EditorSettings;

    #[test]
    fn terminal_profile_controls_are_rejected_on_apply() {
        let mut settings = EditorSettings {
            terminal_shell_path: Some("pwsh.exe".to_owned()),
            terminal_shell_args: vec!["-NoLogo".to_owned()],
            terminal_cwd: Some("workspace".to_owned()),
            ..EditorSettings::default()
        };
        let draft = EditorSettings {
            terminal_shell_path: Some("pwsh.exe\n-NoProfile".to_owned()),
            terminal_shell_args: vec![
                " -NoLogo ".to_owned(),
                "-Command\tbad".to_owned(),
                "-Hidden\u{202e}bad".to_owned(),
                String::new(),
                " ok ".to_owned(),
            ],
            terminal_cwd: Some("tools\u{2028}bad".to_owned()),
            ..EditorSettings::default()
        };

        apply_terminal_settings_draft(&mut settings, &draft);

        assert_eq!(settings.terminal_shell_path, None);
        assert_eq!(
            settings.terminal_shell_args,
            vec![" -NoLogo ".to_owned(), " ok ".to_owned()]
        );
        assert_eq!(settings.terminal_cwd, None);
    }

    #[test]
    fn terminal_tab_text_preserves_raw_non_empty_values_and_falls_back_for_blank() {
        let mut settings = EditorSettings::default();
        let draft = EditorSettings {
            terminal_tabs_default_icon: " code ".to_owned(),
            terminal_tabs_default_color: Some(" terminal.ansiBlue ".to_owned()),
            terminal_tabs_title: " ${process} - ${cwd} ".to_owned(),
            ..EditorSettings::default()
        };

        apply_terminal_settings_draft(&mut settings, &draft);

        assert_eq!(settings.terminal_tabs_default_icon, " code ");
        assert_eq!(
            settings.terminal_tabs_default_color.as_deref(),
            Some(" terminal.ansiBlue ")
        );
        assert_eq!(settings.terminal_tabs_title, " ${process} - ${cwd} ");

        let draft = EditorSettings {
            terminal_tabs_default_icon: " \t ".to_owned(),
            terminal_tabs_default_color: Some(" \t ".to_owned()),
            terminal_tabs_title: " \t ".to_owned(),
            ..EditorSettings::default()
        };

        apply_terminal_settings_draft(&mut settings, &draft);

        assert_eq!(
            settings.terminal_tabs_default_icon,
            kuroya_core::DEFAULT_TERMINAL_TABS_DEFAULT_ICON
        );
        assert_eq!(settings.terminal_tabs_default_color, None);
        assert_eq!(
            settings.terminal_tabs_title,
            kuroya_core::DEFAULT_TERMINAL_TABS_TITLE
        );
    }

    #[test]
    fn terminal_tab_text_strips_hidden_controls_and_bounds_on_apply() {
        let mut settings = EditorSettings::default();
        let draft = EditorSettings {
            terminal_tabs_default_icon: format!("term\u{202e}inal{}", "x".repeat(9000)),
            terminal_tabs_default_color: Some("#fff\u{200b}\nblue".to_owned()),
            terminal_tabs_title: "Proc\u{2028}${cwd}".to_owned(),
            ..EditorSettings::default()
        };

        apply_terminal_settings_draft(&mut settings, &draft);

        assert!(!settings.terminal_tabs_default_icon.contains('\u{202e}'));
        assert!(
            settings.terminal_tabs_default_icon.chars().count() <= MAX_TERMINAL_SETTING_TEXT_CHARS
        );
        assert_eq!(
            settings.terminal_tabs_default_color.as_deref(),
            Some("#fff blue")
        );
        assert_eq!(settings.terminal_tabs_title, "Proc ${cwd}");
    }

    #[test]
    fn terminal_word_separators_strip_hidden_controls_and_bound_on_apply() {
        let mut settings = EditorSettings::default();
        let draft = EditorSettings {
            terminal_word_separators: format!(":\u{202e}\n{}", "x".repeat(9000)),
            ..EditorSettings::default()
        };

        apply_terminal_settings_draft(&mut settings, &draft);

        assert!(settings.terminal_word_separators.starts_with(": "));
        assert!(!settings.terminal_word_separators.contains('\u{202e}'));
        assert!(!settings.terminal_word_separators.contains('\n'));
        assert!(
            settings.terminal_word_separators.chars().count() <= MAX_TERMINAL_SETTING_TEXT_CHARS
        );
    }
}
