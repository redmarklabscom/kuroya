use super::*;

#[test]
fn normal_mode_vim_settings_override_escape_key_to_command() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Esc>".to_owned(),
            after: String::new(),
            command: Some(kuroya_core::Command::RequestHover),
        }],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(result.handled);
    assert!(!result.changed);
    assert_eq!(result.suppress_text, None);
    assert_eq!(result.command, Some(kuroya_core::Command::RequestHover));
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_override_ctrl_open_bracket_escape_alias() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Esc>".to_owned(),
            after: String::new(),
            command: Some(kuroya_core::Command::RequestHover),
        }],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(result.handled);
    assert!(!result.changed);
    assert_eq!(result.command, Some(kuroya_core::Command::RequestHover));
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}
