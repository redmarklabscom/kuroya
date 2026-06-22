use super::*;

#[test]
fn normal_mode_t_and_shift_t_move_until_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "axbxcxd".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let till = handle_vim_editor_key_event(
        &mut buffer,
        Key::T,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let target = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(till.handled);
    assert!(!till.changed);
    assert_eq!(till.suppress_text, Some('t'));
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some('c'));
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::T,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer.set_single_cursor(0);
    for key in [Key::Num2, Key::T, Key::X] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 2);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.len_chars().saturating_sub(1));
    let till_back = handle_vim_editor_key_event(
        &mut buffer,
        Key::T,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    let backward_target = handle_vim_editor_key_event(
        &mut buffer,
        Key::B,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(till_back.handled);
    assert_eq!(till_back.suppress_text, Some('T'));
    assert!(backward_target.handled);
    assert_eq!(backward_target.suppress_text, Some('b'));
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.len_chars().saturating_sub(1));
    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::T, Modifiers::SHIFT),
        (Key::X, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_semicolon_and_comma_repeat_char_finds() {
    let mut buffer = TextBuffer::from_text(1, None, "axbxcxd".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::F, Key::X] {
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

    assert_eq!(buffer.cursor(), 1);
    assert!(last_char_find.is_some());

    let repeat = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Semicolon,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(repeat.suppress_text, Some(';'));
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Semicolon] {
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

    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    buffer.set_single_cursor(1);
    for key in [Key::Num2, Key::Semicolon] {
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

    assert_eq!(buffer.cursor(), 5);
    assert!(pending.is_none());

    let reverse = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Comma,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(reverse.suppress_text, Some(','));
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Semicolon,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Comma,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
