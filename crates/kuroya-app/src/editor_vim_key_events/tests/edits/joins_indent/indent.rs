use super::*;

#[test]
fn normal_mode_shift_period_and_shift_comma_indent_outdent_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let first_indent = handle_vim_editor_key_event_with_state_and_indent(
        &mut buffer,
        Key::Period,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        "  ",
    );
    let second_indent = handle_vim_editor_key_event_with_state_and_indent(
        &mut buffer,
        Key::Period,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        "  ",
    );

    assert!(first_indent.handled);
    assert!(!first_indent.changed);
    assert_eq!(first_indent.suppress_text, Some('>'));
    assert!(second_indent.handled);
    assert!(second_indent.changed);
    assert_eq!(second_indent.suppress_text, Some('>'));
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "  one\ntwo\nthree\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 3));

    buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    mode = EditorVimMode::Normal;
    pending = None;

    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::Period, Modifiers::SHIFT),
        (Key::Period, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_state_and_indent(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
            "  ",
        );
        assert!(result.handled);
    }

    assert!(pending.is_none());
    assert_eq!(buffer.text(), "one\n  two\n  three\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 3));

    buffer = TextBuffer::from_text(1, None, "\tone\n    two\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    mode = EditorVimMode::Normal;
    pending = None;

    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::Comma, Modifiers::SHIFT),
        (Key::Comma, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_state_and_indent(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
            "  ",
        );
        assert!(result.handled);
    }

    assert!(pending.is_none());
    assert_eq!(buffer.text(), "one\n  two\nthree\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Period,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Key {
                key: Key::Period,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Comma,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Key {
                key: Key::Comma,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
