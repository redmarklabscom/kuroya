use super::*;

#[test]
fn insert_mode_vim_settings_disable_escape_binding() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["<Esc>".to_owned()],
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
    assert_eq!(result.command, None);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
}

#[test]
fn insert_mode_vim_settings_override_escape_to_command() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Insert;
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
    assert_eq!(result.command, Some(kuroya_core::Command::RequestHover));
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
}

#[test]
fn insert_mode_vim_settings_override_ctrl_open_bracket_escape_alias() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Insert;
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
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
}

#[test]
fn insert_mode_vim_settings_remap_escape_to_escape_exits_insert() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Esc>".to_owned(),
            after: "<Esc>".to_owned(),
            command: None,
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
    assert_eq!(result.command, None);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn insert_mode_vim_settings_remapped_escape_key_exits_insert() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["<Esc>".to_owned()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "q".to_owned(),
            after: "<Esc>".to_owned(),
            command: None,
        }],
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::Q,
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
    assert_eq!(result.suppress_text, Some('q'));
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn insert_mode_vim_settings_remap_escape_to_normal_sequence() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Esc>".to_owned(),
            after: "x".to_owned(),
            command: None,
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
    assert!(result.changed);
    assert_eq!(result.command, None);
    assert_eq!(buffer.text(), "apha");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}
