use super::*;

#[test]
fn normal_mode_hjkl_moves_without_editing() {
    let mut buffer = TextBuffer::from_text(1, None, "abc\ndef\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let right = handle_vim_editor_key_event(
        &mut buffer,
        Key::L,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::J,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(right.handled);
    assert!(down.handled);
    assert!(!right.changed);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, 1);
    assert_eq!(buffer.cursor_position().column, 2);
    assert_eq!(buffer.text(), "abc\ndef\n");
}

#[test]
fn normal_mode_space_and_backspace_move_without_editing() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(2);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let space = handle_vim_editor_key_event(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(space.handled);
    assert!(!space.changed);
    assert_eq!(space.suppress_text, Some(' '));
    assert_eq!(buffer.cursor(), 3);

    for key in [Key::Num2, Key::Space] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), 5);

    let backspace = handle_vim_editor_key_event(
        &mut buffer,
        Key::Backspace,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(backspace.handled);
    assert!(!backspace.changed);
    assert_eq!(buffer.cursor(), 4);
    assert_eq!(buffer.text(), "abcdef");
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(" ".to_owned()),
            Event::Key {
                key: Key::Backspace,
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
fn normal_mode_space_and_backspace_wrap_lines_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "ab\ncd\nef".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let space = handle_vim_editor_key_event(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(space.handled);
    assert!(!space.changed);
    assert_eq!(space.suppress_text, Some(' '));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Space] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 0));
    assert!(pending.is_none());

    let backspace = handle_vim_editor_key_event(
        &mut buffer,
        Key::Backspace,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(backspace.handled);
    assert!(!backspace.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 1));
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Backspace] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 1));
    assert_eq!(buffer.text(), "ab\ncd\nef");
    assert!(pending.is_none());
}

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
fn normal_mode_counts_repeat_motions_and_delete_forward() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let count = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let right = handle_vim_editor_key_event(
        &mut buffer,
        Key::L,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(count.handled);
    assert_eq!(count.suppress_text, Some('3'));
    assert!(right.handled);
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    let count = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num2,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let delete = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(count.handled);
    assert!(delete.changed);
    assert_eq!(buffer.text(), "alp beta gamma");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_e_moves_to_word_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(end.handled);
    assert_eq!(end.suppress_text, Some('e'));
    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::E] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 10);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_ge_moves_to_previous_word_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let previous_end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(go.handled);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(previous_end.handled);
    assert_eq!(previous_end.suppress_text, Some('e'));
    assert_eq!(buffer.cursor(), 15);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::G, Key::E] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_g_shift_e_moves_to_previous_big_word_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let previous_end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(go.handled);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(previous_end.handled);
    assert_eq!(previous_end.suppress_text, Some('E'));
    assert_eq!(buffer.cursor(), 22);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::G, Key::E] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::E {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_big_word_motions_and_operator_forms_use_whitespace_groups() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    let big_word = handle_vim_editor_key_event(
        &mut buffer,
        Key::W,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(big_word.handled);
    assert_eq!(big_word.suppress_text, Some('W'));
    assert_eq!(buffer.cursor(), 6);

    buffer.set_single_cursor(0);
    for (key, modifiers) in [(Key::Num2, Modifiers::NONE), (Key::W, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor(), 18);

    buffer.set_single_cursor(0);
    let big_end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(big_end.handled);
    assert_eq!(big_end.suppress_text, Some('E'));
    assert_eq!(buffer.cursor(), 4);

    buffer.set_single_cursor(buffer.len_chars());
    let big_back = handle_vim_editor_key_event(
        &mut buffer,
        Key::B,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(big_back.handled);
    assert_eq!(big_back.suppress_text, Some('B'));
    assert_eq!(buffer.cursor(), 18);
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(0);
    mode = EditorVimMode::Normal;
    pending = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::W, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "beta.gamma  delta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alpha ")
    );

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::B, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "alpha gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta.")
    );

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::C, Modifiers::NONE), (Key::E, Modifiers::SHIFT)] {
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), " beta.gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alpha")
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
                key: Key::W,
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
                key: Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::E,
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
fn normal_mode_percent_moves_and_operates_on_matching_brackets() {
    let mut buffer = TextBuffer::from_text(1, None, "(alpha [beta]) tail".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    let forward = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num5,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(forward.handled);
    assert!(!forward.changed);
    assert_eq!(forward.suppress_text, Some('%'));
    assert_eq!(buffer.cursor(), 13);

    let backward = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num5,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(backward.handled);
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());

    buffer.set_single_cursor(7);
    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::Num5, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "(alpha ) tail");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("[beta]")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "(alpha [beta]) tail".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::C, Modifiers::NONE), (Key::Num5, Modifiers::SHIFT)] {
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), " tail");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("(alpha [beta])")
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
                key: Key::Num5,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[Event::Key {
            key: Key::Num5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));
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
fn normal_mode_marks_set_and_jump_linewise_or_exact() {
    let mut buffer = TextBuffer::from_text(1, None, "zero\n    alpha beta\nthree\n".to_owned());
    let marked_cursor = buffer.line_column_to_char(1, 10);
    buffer.set_single_cursor(marked_cursor);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let mark = handle_vim_editor_key_event(
        &mut buffer,
        Key::M,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(mark.handled);
    assert_eq!(mark.suppress_text, Some('m'));
    let mark_name = handle_vim_editor_key_event(
        &mut buffer,
        Key::A,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(mark_name.handled);
    assert_eq!(mark_name.suppress_text, Some('a'));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for key in [Key::Quote, Key::A] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for key in [Key::Backtick, Key::A] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), marked_cursor);
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::M,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Quote,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::A,
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
fn normal_mode_zero_stays_line_start_unless_count_is_active() {
    let mut buffer = TextBuffer::from_text(1, None, "zero\none\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(2, 2));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let zero = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num0,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(zero.handled);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 0));

    for key in [Key::Num1, Key::Num0, Key::K] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor_position().line, 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_line_start_keys_use_vim_semantics() {
    let mut buffer = TextBuffer::from_text(1, None, "    let value = 1;".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let caret = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num6,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(caret.handled);
    assert_eq!(caret.suppress_text, Some('^'));
    assert_eq!(buffer.cursor_position().column, 4);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
    let zero = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num0,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(zero.handled);
    assert_eq!(zero.suppress_text, Some('0'));
    assert_eq!(buffer.cursor_position().column, 0);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
    let insert = handle_vim_editor_key_event(
        &mut buffer,
        Key::I,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(insert.handled);
    assert_eq!(insert.suppress_text, Some('I'));
    assert_eq!(buffer.cursor_position().column, 4);
    assert_eq!(mode, EditorVimMode::Insert);
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
fn normal_mode_plus_minus_enter_and_underscore_move_to_first_non_whitespace() {
    let mut buffer = TextBuffer::from_text(1, None, "root\n    child\n\tleaf\nlast\n".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let plus = handle_vim_editor_key_event(
        &mut buffer,
        Key::Equals,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(plus.handled);
    assert!(!plus.changed);
    assert_eq!(plus.suppress_text, Some('+'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::Equals, Modifiers::SHIFT),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 1));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let enter = handle_vim_editor_key_event(
        &mut buffer,
        Key::Enter,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(enter.handled);
    assert!(!enter.changed);
    assert_eq!(enter.suppress_text, None);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(2, 3));
    let minus = handle_vim_editor_key_event(
        &mut buffer,
        Key::Minus,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(minus.handled);
    assert!(!minus.changed);
    assert_eq!(minus.suppress_text, Some('-'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let underscore = handle_vim_editor_key_event(
        &mut buffer,
        Key::Minus,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(underscore.handled);
    assert!(!underscore.changed);
    assert_eq!(underscore.suppress_text, Some('_'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    for (key, modifiers) in [(Key::Num3, Modifiers::NONE), (Key::Minus, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 1));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Equals,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Text("+".to_owned()),
            Event::Key {
                key: Key::Minus,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("-".to_owned()),
            Event::Key {
                key: Key::Minus,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Text("_".to_owned()),
            Event::Key {
                key: Key::Enter,
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

#[test]
fn normal_mode_home_and_end_mirror_line_start_and_end_motions() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo words\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 4));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let home = handle_vim_editor_key_event(
        &mut buffer,
        Key::Home,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(home.handled);
    assert!(!home.changed);
    assert_eq!(home.suppress_text, None);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());

    for key in [Key::Num2, Key::End] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_content_end_char(2));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Home,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::End,
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
