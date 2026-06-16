use super::*;

#[test]
fn insert_mode_escape_returns_to_normal() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Normal);
}

#[test]
fn insert_mode_ctrl_open_bracket_returns_to_normal() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::OpenBracket,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Insert,
        None,
    ));
}

#[test]
fn insert_mode_ctrl_h_deletes_previous_char() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(2);
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::H,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "ac");
    assert_eq!(buffer.cursor(), 1);
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::H,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        EditorVimMode::Insert,
        None,
    ));
}

#[test]
fn insert_mode_ctrl_u_deletes_to_line_start() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha\nbeta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "alpha\ngamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::U,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        EditorVimMode::Insert,
        None,
    ));

    let no_op = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(no_op.handled);
    assert!(!no_op.changed);
    assert_eq!(buffer.text(), "alpha\ngamma");
}

#[test]
fn insert_mode_ctrl_w_deletes_previous_word() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::W,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "alpha gamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::W,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        EditorVimMode::Insert,
        None,
    ));
}

#[test]
fn normal_mode_period_replays_insert_mode_ctrl_u_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "prefix-tail\nagain-more".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);

    buffer.insert_at_cursors("abc");
    vim_record_inserted_text(&mut last_change, "abc");
    let delete_line_backward = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(delete_line_backward.handled);
    assert!(delete_line_backward.changed);
    buffer.insert_at_cursors("X");
    vim_record_inserted_text(&mut last_change, "X");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "X-tail\nagain-more");

    buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "X-tail\nX-more");
}

#[test]
fn normal_mode_period_replays_insert_mode_ctrl_w_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "one two-tail\nred blue-more".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);

    let delete_word_backward = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::W,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(delete_word_backward.handled);
    assert!(delete_word_backward.changed);
    buffer.insert_at_cursors("new");
    vim_record_inserted_text(&mut last_change, "new");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "one new-tail\nred blue-more");

    buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "one new-tail\nred new-more");
}

#[test]
fn normal_mode_period_replays_insert_mode_ctrl_h_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "ab-cd\nxy-zw".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);

    buffer.insert_at_cursors("q");
    vim_record_inserted_text(&mut last_change, "q");
    let delete_char_backward = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::H,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(delete_char_backward.handled);
    assert!(delete_char_backward.changed);
    buffer.insert_at_cursors("R");
    vim_record_inserted_text(&mut last_change, "R");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "abR-cd\nxy-zw");

    buffer.set_single_cursor(buffer.line_column_to_char(1, 2));
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "abR-cd\nxyR-zw");
}

#[test]
fn normal_mode_ctrl_open_bracket_clears_pending_key() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = Some(EditorVimPendingKey::DeleteLine(1));

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_i_enters_insert_and_suppresses_i_text() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let mut suppressed = VecDeque::from([result.suppress_text.unwrap()]);

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(
        vim_text_after_suppression("iabc", &mut suppressed).as_deref(),
        Some("abc")
    );
}

#[test]
fn normal_mode_prescan_suppresses_printable_insert_command_text() {
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::I,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("i".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::I,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::I,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("ix".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_prescan_suppresses_printable_pending_key_text() {
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("f".to_owned()),
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_prescan_detects_mutating_printable_pending_key() {
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("r".to_owned()),
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_o_and_shift_o_open_indented_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "  one\n  two\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 3));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    assert_eq!(vim_open_line_below_text("\t  "), "\n\t  ");
    assert_eq!(vim_open_line_above_text("\t  "), "\t  \n");

    let open_below = handle_vim_editor_key_event(
        &mut buffer,
        Key::O,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(open_below.handled);
    assert!(open_below.changed);
    assert_eq!(open_below.suppress_text, Some('o'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "  one\n  \n  two\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 2));

    buffer = TextBuffer::from_text(1, None, "  one\n  two\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 3));
    mode = EditorVimMode::Normal;
    pending = None;

    let open_above = handle_vim_editor_key_event(
        &mut buffer,
        Key::O,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(open_above.handled);
    assert!(open_above.changed);
    assert_eq!(open_above.suppress_text, Some('O'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "  one\n  \n  two\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 2));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::O,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::O,
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
fn normal_mode_period_repeats_last_change_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::X, Key::Period] {
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
    assert_eq!(buffer.text(), "cdef");
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    last_change = None;

    for key in [Key::X, Key::Num3, Key::Period] {
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
    assert_eq!(buffer.text(), "ef");
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for key in [Key::D, Key::A, Key::W, Key::Period] {
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
    assert_eq!(buffer.text(), "gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta ")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for key in [Key::C, Key::A, Key::W] {
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
    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
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
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta ")
    );

    buffer = TextBuffer::from_text(1, None, "ab".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    buffer.insert_at_cursors("X");
    vim_record_inserted_text(&mut last_change, "X");
    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "XXab");

    buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    let substitute = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::S,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(substitute.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    buffer.insert_at_cursors("X");
    vim_record_inserted_text(&mut last_change, "X");
    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    buffer.set_single_cursor(1);
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "XXc");

    buffer = TextBuffer::from_text(1, None, String::new());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    buffer.insert_at_cursors("a");
    vim_record_inserted_text(&mut last_change, "a");
    assert!(buffer.delete_backward_with_auto_pair_delete(false));
    vim_record_insert_replay_key_with_auto_indent(
        &mut last_change,
        Key::Backspace,
        Modifiers::NONE,
        false,
    );
    buffer.insert_at_cursors("b");
    vim_record_inserted_text(&mut last_change, "b");
    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "bb");

    buffer = TextBuffer::from_text(1, None, String::new());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    buffer.insert_at_cursors("a");
    vim_record_inserted_text(&mut last_change, "a");
    buffer.insert_at_cursors("\n");
    vim_record_insert_replay_key_with_auto_indent(
        &mut last_change,
        Key::Enter,
        Modifiers::NONE,
        false,
    );
    buffer.insert_at_cursors("    ");
    vim_record_insert_replay_key_with_auto_indent(
        &mut last_change,
        Key::Tab,
        Modifiers::NONE,
        false,
    );
    buffer.insert_at_cursors("b");
    vim_record_inserted_text(&mut last_change, "b");
    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "a\n    ba\n    b");

    buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::R, Modifiers::NONE),
        (Key::X, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::Period, Modifiers::NONE),
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
    assert_eq!(buffer.text(), "xxc");
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::Period,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Num2,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Period,
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
fn normal_mode_period_replays_auto_indented_insert_enter() {
    let mut buffer = TextBuffer::from_text(1, None, "if ready {".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let append_line_end = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::A,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(append_line_end.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    buffer.insert_newline_with_indent_unit("  ");
    vim_record_insert_replay_key_with_auto_indent(
        &mut last_change,
        Key::Enter,
        Modifiers::NONE,
        true,
    );
    buffer.insert_at_cursors("x");
    vim_record_inserted_text(&mut last_change, "x");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
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
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "if ready {\n  x\n  x");
}
