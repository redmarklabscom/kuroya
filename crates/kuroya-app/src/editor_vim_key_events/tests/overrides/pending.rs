use super::*;

#[test]
fn normal_mode_vim_settings_disable_pending_operator_motion() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["w".to_owned()],
        ..EditorVimSettings::default()
    };

    let delete = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );
    let word = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::W,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(delete.handled);
    assert!(!delete.changed);
    assert!(word.handled);
    assert!(!word.changed);
    assert_eq!(word.suppress_text, Some('w'));
    assert_eq!(buffer.text(), "alpha beta");
    assert!(matches!(pending, Some(EditorVimPendingKey::DeleteLine(1))));
}

#[test]
fn normal_mode_vim_settings_remap_pending_operator_motion() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "w".to_owned(),
            after: "l".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    let delete = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );
    let remapped_word = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::W,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(delete.handled);
    assert!(!delete.changed);
    assert!(remapped_word.handled);
    assert!(remapped_word.changed);
    assert_eq!(remapped_word.suppress_text, Some('w'));
    assert_eq!(buffer.text(), "lpha beta");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_do_not_intercept_command_or_search_input_text() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = Some(EditorVimPendingKey::CommandInput);
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["w".to_owned()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "s".to_owned(),
            after: "x".to_owned(),
            command: None,
        }],
    };

    super::super::super::vim_clear_command_input();
    let command_w = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::W,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(command_w.handled);
    assert!(!command_w.changed);
    assert_eq!(command_w.suppress_text, Some('w'));
    assert_eq!(
        vim_pending_command_status_label(pending).as_deref(),
        Some(":w")
    );

    super::super::super::vim_clear_search_input();
    pending = Some(EditorVimPendingKey::SearchInput {
        count: 1,
        forward: true,
    });
    let search_s = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::S,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(search_s.handled);
    assert!(!search_s.changed);
    assert_eq!(search_s.suppress_text, Some('s'));
    assert_eq!(
        vim_pending_search_status_label(pending).as_deref(),
        Some("/s")
    );
}
