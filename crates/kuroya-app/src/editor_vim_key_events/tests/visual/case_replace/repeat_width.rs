use super::*;

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
