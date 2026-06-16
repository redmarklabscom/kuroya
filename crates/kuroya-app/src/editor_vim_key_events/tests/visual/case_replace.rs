use super::*;

#[test]
fn normal_mode_visual_character_case_commands_convert_selection() {
    let mut buffer = TextBuffer::from_text(1, None, "aBcD ef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = Some(EditorVimRegister {
        text: "kept".to_owned(),
        kind: EditorVimRegisterKind::Characterwise,
    });

    for key in [Key::V, Key::Num2, Key::L] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("BcD"));
    let lower = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::U,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(lower.handled);
    assert!(lower.changed);
    assert_eq!(lower.suppress_text, Some('u'));
    assert_eq!(buffer.text(), "abcd ef");
    assert_eq!(buffer.cursor(), 1);
    assert!(!buffer.has_selection());
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("kept", EditorVimRegisterKind::Characterwise))
    );

    buffer = TextBuffer::from_text(1, None, "abCd".to_owned());
    buffer.set_single_cursor(3);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::V, Key::H] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("Cd"));
    let upper = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::U,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(upper.handled);
    assert!(upper.changed);
    assert_eq!(upper.suppress_text, Some('U'));
    assert_eq!(buffer.text(), "abCD");
    assert_eq!(buffer.cursor(), 2);
    assert!(!buffer.has_selection());
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "aBcD".to_owned());
    buffer.set_single_cursor(1);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::V, Key::Num2, Key::L] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("BcD"));
    let toggle = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Backtick,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(toggle.handled);
    assert!(toggle.changed);
    assert_eq!(toggle.suppress_text, Some('~'));
    assert_eq!(buffer.text(), "abCd");
    assert_eq!(buffer.cursor(), 1);
    assert!(!buffer.has_selection());
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::U, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::Backtick, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_visual_character_case_commands_repeat_selected_width() {
    let mut buffer = TextBuffer::from_text(1, None, "ABcd EFgh".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::Num3, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::U, Modifiers::NONE),
    ] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "abcd EFgh");
    assert_eq!(buffer.cursor(), 0);
    assert!(!buffer.has_selection());
    assert!(pending.is_none());
    assert!(last_change.is_some());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "abcd efgh");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 5));
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abCD efGH".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::Num3, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::U, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "ABCD efGH");
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "ABCD EFGH");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 5));
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "aBcD eFgH".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::Num3, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::Backtick, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "AbCd eFgH");
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "AbCd EfGh");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 5));
    assert!(pending.is_none());
}

#[test]
fn normal_mode_visual_character_r_replaces_selection_with_printable_char() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef ghij".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::V, Key::Num2, Key::L] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("bcd"));
    let replace = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::R,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(replace.handled);
    assert!(!replace.changed);
    assert_eq!(replace.suppress_text, Some('r'));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterReplace {
            anchor: 1,
            cursor: 3,
        })
    );
    assert_eq!(buffer.selected_text().as_deref(), Some("bcd"));

    let target = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::X,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(target.handled);
    assert!(target.changed);
    assert_eq!(target.suppress_text, Some('X'));
    assert_eq!(buffer.text(), "aXXXef ghij");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert!(!buffer.has_selection());
    assert!(unnamed_register.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "aXXXef XXXj");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 9));
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::R, Modifiers::NONE),
            key_event(Key::X, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::R, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
