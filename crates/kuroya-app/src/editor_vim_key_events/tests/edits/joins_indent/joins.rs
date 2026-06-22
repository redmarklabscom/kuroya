use super::*;

#[test]
fn normal_mode_shift_j_joins_lines_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "one\n  two\nthree\n".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let join = handle_vim_editor_key_event(
        &mut buffer,
        Key::J,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(join.handled);
    assert!(join.changed);
    assert_eq!(join.suppress_text, Some('J'));
    assert_eq!(buffer.text(), "one two\nthree\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::J,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::J] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::J {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "one two three\nfour\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_g_shift_j_joins_lines_without_whitespace() {
    let mut buffer = TextBuffer::from_text(1, None, "one\n  two\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let go = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    let join = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::J,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(go.handled);
    assert!(!go.changed);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(join.handled);
    assert!(join.changed);
    assert_eq!(join.suppress_text, Some('J'));
    assert_eq!(buffer.text(), "onetwo\nthree\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 1));
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::G,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::J,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "a\n  b\n\tc\nd\n".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::J, Modifiers::SHIFT),
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

    assert_eq!(buffer.text(), "abc\nd\n");
    assert!(pending.is_none());

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
    assert_eq!(buffer.text(), "abcd\n");
    assert!(pending.is_none());
}
