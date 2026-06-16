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

#[test]
fn normal_mode_x_deletes_forward_and_counts_as_mutation() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(buffer.text(), "ac");
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::X,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_shift_x_deletes_backward_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(3);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(result.suppress_text, Some('X'));
    assert_eq!(buffer.text(), "abdef");
    assert_eq!(buffer.cursor(), 2);
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::X,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(4);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num3, Key::X] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::X {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "aef");
    assert_eq!(buffer.cursor(), 1);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_dd_deletes_current_line_after_pending_key() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let first = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let second = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(first.handled);
    assert!(!first.changed);
    assert!(second.changed);
    assert_eq!(buffer.text(), "one\nthree\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        Some(super::EditorVimPendingKey::DeleteLine(1)),
    ));
}

#[test]
fn normal_mode_shift_d_deletes_to_line_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta\ngamma delta\nomega\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let delete = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(delete.handled);
    assert!(delete.changed);
    assert_eq!(delete.suppress_text, Some('D'));
    assert_eq!(buffer.text(), "alpha \ngamma delta\nomega\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    for key in [Key::Num2, Key::D] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::D {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "alpha \nomega\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_shift_c_changes_to_line_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta\ngamma delta\nomega\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let change = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(change.handled);
    assert!(change.changed);
    assert_eq!(change.suppress_text, Some('C'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha \ngamma delta\nomega\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "alpha beta\ngamma delta\nomega\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::C] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::C {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha \nomega\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_cc_changes_current_line_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let first = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let second = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(first.handled);
    assert!(!first.changed);
    assert_eq!(first.suppress_text, Some('c'));
    assert!(second.changed);
    assert_eq!(second.suppress_text, Some('c'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\nthree\nfour\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        Some(super::EditorVimPendingKey::ChangeLine(1)),
    ));

    buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::C, Key::C] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\nfour\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_s_substitutes_forward_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let substitute = handle_vim_editor_key_event(
        &mut buffer,
        Key::S,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(substitute.handled);
    assert!(substitute.changed);
    assert_eq!(substitute.suppress_text, Some('s'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "acdef");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num3, Key::S] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "aef");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_r_replaces_forward_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let replace = handle_vim_editor_key_event(
        &mut buffer,
        Key::R,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let replacement = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(replace.handled);
    assert!(!replace.changed);
    assert_eq!(replace.suppress_text, Some('r'));
    assert_eq!(pending, None);
    assert!(replacement.handled);
    assert!(replacement.changed);
    assert_eq!(replacement.suppress_text, Some('x'));
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "axcdef");
    assert_eq!(buffer.cursor(), 1);
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
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
            }
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num3, Key::R, Key::X] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "axxxef");
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_r_accepts_shifted_punctuation_replacements() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let replace = handle_vim_editor_key_event(
        &mut buffer,
        Key::R,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let replacement = handle_vim_editor_key_event(
        &mut buffer,
        Key::Quote,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(replace.handled);
    assert_eq!(replace.suppress_text, Some('r'));
    assert!(replacement.handled);
    assert!(replacement.changed);
    assert_eq!(replacement.suppress_text, Some('"'));
    assert_eq!(buffer.text(), "a\"c");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
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
                modifiers: Modifiers::SHIFT,
            }
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_r_accepts_enter_replacement_and_repeats() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

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
    let replacement = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Enter,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(replace.handled);
    assert!(!replace.changed);
    assert!(replacement.handled);
    assert!(replacement.changed);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "a\ncdef");
    assert!(pending.is_none());
    assert!(last_change.is_some());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer.set_single_cursor(buffer.line_column_to_char(1, 0));
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
    assert_eq!(buffer.text(), "a\n\ndef");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_shift_s_changes_current_line_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let change = handle_vim_editor_key_event(
        &mut buffer,
        Key::S,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(change.handled);
    assert!(change.changed);
    assert_eq!(change.suppress_text, Some('S'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\nthree\nfour\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::S] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::S {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\nfour\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_counted_dd_deletes_multiple_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for key in [Key::Num2, Key::D, Key::D] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "one\nfour\n");
    assert!(pending.is_none());
}
