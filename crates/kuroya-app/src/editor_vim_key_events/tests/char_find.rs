use super::*;

#[test]
fn normal_mode_space_can_be_char_find_target() {
    let mut buffer = TextBuffer::from_text(1, None, "ab cd ef".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let find = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let target = handle_vim_editor_key_event(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(find.handled);
    assert!(!find.changed);
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some(' '));
    assert_eq!(buffer.cursor(), 2);
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
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(" ".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_char_find_operator_motions_delete_change_yank_and_repeat() {
    let mut buffer = TextBuffer::from_text(1, None, "abxcdx".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::D, Key::F, Key::X] {
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
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "cdx");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("abx", EditorVimRegisterKind::Characterwise))
    );
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
    assert_eq!(buffer.text(), "");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("cdx", EditorVimRegisterKind::Characterwise))
    );

    buffer = TextBuffer::from_text(1, None, "abxcdxzz".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for key in [Key::D, Key::Num2, Key::F, Key::X] {
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
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "zz");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("abxcdx", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abcxdef".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for key in [Key::C, Key::T, Key::X] {
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
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "xdef");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("abc", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abcxdef".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [
        (Key::D, Modifiers::NONE),
        (Key::T, Modifiers::SHIFT),
        (Key::X, Modifiers::NONE),
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

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "abcxf");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("de", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abcxdef".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [
        (Key::Y, Modifiers::NONE),
        (Key::F, Modifiers::SHIFT),
        (Key::X, Modifiers::NONE),
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
        assert!(!result.changed);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "abcxdef");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("xde", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
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
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
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
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Y,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
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
}

#[test]
fn normal_mode_f_and_shift_f_find_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "axbxcxd".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let find = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let target = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(find.handled);
    assert!(!find.changed);
    assert_eq!(find.suppress_text, Some('f'));
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some('x'));
    assert_eq!(buffer.cursor(), 1);
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
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer.set_single_cursor(0);
    for key in [Key::Num2, Key::F, Key::X] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.len_chars().saturating_sub(1));
    let find_back = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    let backward_target = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(find_back.handled);
    assert_eq!(find_back.suppress_text, Some('F'));
    assert!(backward_target.handled);
    assert_eq!(backward_target.suppress_text, Some('x'));
    assert_eq!(buffer.cursor(), 5);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.len_chars().saturating_sub(1));
    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::F, Modifiers::SHIFT),
        (Key::X, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_f_accepts_shifted_punctuation_targets() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha:beta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let find = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let target = handle_vim_editor_key_event(
        &mut buffer,
        Key::Semicolon,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(find.handled);
    assert_eq!(find.suppress_text, Some('f'));
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some(':'));
    assert_eq!(buffer.cursor(), 5);
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
                key: Key::Semicolon,
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
fn normal_mode_f_accepts_direct_punctuation_key_targets() {
    for (key, target, cursor) in [
        (Key::Colon, ':', 1),
        (Key::Plus, '+', 3),
        (Key::Questionmark, '?', 5),
        (Key::Exclamationmark, '!', 7),
        (Key::OpenCurlyBracket, '{', 9),
        (Key::CloseCurlyBracket, '}', 11),
    ] {
        let mut buffer = TextBuffer::from_text(1, None, "a:b+c?d!e{f}".to_owned());
        let mut mode = EditorVimMode::Normal;
        let mut pending = None;

        let find = handle_vim_editor_key_event(
            &mut buffer,
            Key::F,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
        );
        let found =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);

        assert!(find.handled);
        assert!(!find.changed);
        assert_eq!(find.suppress_text, Some('f'));
        assert!(found.handled);
        assert!(!found.changed);
        assert_eq!(found.suppress_text, Some(target));
        assert_eq!(buffer.cursor(), cursor);
        assert!(pending.is_none());
        assert!(!vim_events_include_mutation(
            &[
                key_event(Key::F, Modifiers::NONE),
                key_event(key, Modifiers::NONE)
            ],
            EditorVimMode::Normal,
            None,
        ));
    }
}

#[test]
fn normal_mode_char_find_scans_by_buffer_indices_on_current_line() {
    let mut buffer = TextBuffer::from_text(1, None, "a\u{e9}x\n\u{e9}x".to_owned());

    buffer.set_single_cursor(0);
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::FindForward,
        'x'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 2));

    assert!(!vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::FindForward,
        'x'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 2));

    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::FindBackward,
        '\u{e9}'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));

    buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::TillForward,
        'x'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 1));

    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::TillBackward,
        '\u{e9}'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 1));
}

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
