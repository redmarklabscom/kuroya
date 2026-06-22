use super::*;

#[test]
fn normal_mode_vim_settings_disable_default_binding() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["x".to_owned()],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::X,
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
    assert_eq!(result.suppress_text, Some('x'));
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_disabled_binding_wins_over_override() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["<Esc>".to_owned()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Esc>".to_owned(),
            after: String::new(),
            command: Some(kuroya_core::Command::RequestHover),
        }],
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
    assert_eq!(result.command, None);
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_disable_ctrl_default_binding() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha\nbeta\ngamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["<C-n>".to_owned()],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::N,
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
    assert_eq!(result.suppress_text, None);
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}
