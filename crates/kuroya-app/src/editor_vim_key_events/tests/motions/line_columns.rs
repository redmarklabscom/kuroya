use super::*;

#[test]
fn normal_mode_gg_moves_to_file_start() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(2, 2));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let first = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let second = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(first.handled);
    assert!(!first.changed);
    assert!(second.handled);
    assert!(!second.changed);
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_counted_g_and_gg_jump_to_line() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for key in [Key::Num3, Key::G] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }
    let second_g = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(second_g.handled);
    assert_eq!(buffer.cursor_position().line, 2);

    handle_vim_editor_key_event(
        &mut buffer,
        Key::Num2,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let shift_g = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(shift_g.handled);
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_pipe_moves_to_counted_column_and_operates() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef\nxy\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let pipe = handle_vim_editor_key_event(
        &mut buffer,
        Key::Backslash,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(pipe.handled);
    assert!(!pipe.changed);
    assert_eq!(pipe.suppress_text, Some('|'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));
    assert!(pending.is_none());

    for (key, modifiers) in [
        (Key::Num4, Modifiers::NONE),
        (Key::Backslash, Modifiers::SHIFT),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 3));
    assert_eq!(buffer.text(), "abcdef\nxy\n");
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Num4,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("4".to_owned()),
            Event::Key {
                key: Key::Backslash,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Text("|".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));

    let mut last_char_find = None;
    let mut unnamed_register = None;
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    for (key, modifiers) in [
        (Key::D, Modifiers::NONE),
        (Key::Num5, Modifiers::NONE),
        (Key::Backslash, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.text(), "aef\nxy\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bcd", EditorVimRegisterKind::Characterwise))
    );
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::D,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Backslash,
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

#[test]
fn normal_mode_dollar_moves_to_counted_line_end() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo words\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let end = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num4,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(end.handled);
    assert_eq!(end.suppress_text, Some('$'));
    assert_eq!(buffer.cursor(), buffer.line_content_end_char(0));
    assert!(pending.is_none());

    buffer.set_single_cursor(0);
    for key in [Key::Num3, Key::Num4] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::Num4 {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), buffer.line_content_end_char(2));
    assert!(pending.is_none());
}
