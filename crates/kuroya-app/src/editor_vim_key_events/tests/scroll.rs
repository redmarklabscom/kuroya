use super::*;

#[test]
fn normal_mode_ctrl_d_and_ctrl_u_move_vertically_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..24)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(down.handled);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, VIM_DEFAULT_CTRL_SCROLL_LINES);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::D] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::D {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(
        buffer.cursor_position().line,
        VIM_DEFAULT_CTRL_SCROLL_LINES + 3
    );
    assert!(pending.is_none());

    let up = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(up.handled);
    assert!(!up.changed);
    assert_eq!(buffer.cursor_position().line, 3);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::U] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::U {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_ctrl_e_and_ctrl_y_move_one_line_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..12)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(down.handled);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::E] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::E {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 4);
    assert!(pending.is_none());

    let up = handle_vim_editor_key_event(
        &mut buffer,
        Key::Y,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(up.handled);
    assert!(!up.changed);
    assert_eq!(buffer.cursor_position().line, 3);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Y] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::Y {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::E, Modifiers::CTRL),
            key_event(Key::Num3, Modifiers::NONE),
            key_event(Key::E, Modifiers::CTRL),
            key_event(Key::Y, Modifiers::CTRL),
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::Y, Modifiers::CTRL),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_ctrl_n_and_ctrl_p_move_vertically_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..12)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(down.handled);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());

    for key in [Key::Num4, Key::N] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::N {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 5);
    assert!(pending.is_none());

    let up = handle_vim_editor_key_event(
        &mut buffer,
        Key::P,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(up.handled);
    assert!(!up.changed);
    assert_eq!(buffer.cursor_position().line, 4);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::P] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::P {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::N, Modifiers::CTRL),
            key_event(Key::Num4, Modifiers::NONE),
            key_event(Key::N, Modifiers::CTRL),
            key_event(Key::P, Modifiers::CTRL),
            key_event(Key::Num3, Modifiers::NONE),
            key_event(Key::P, Modifiers::CTRL),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_ctrl_f_and_ctrl_b_move_by_pages_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..96)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let page_down = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(page_down.handled);
    assert!(!page_down.changed);
    assert_eq!(buffer.cursor_position().line, VIM_DEFAULT_PAGE_SCROLL_LINES);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::F] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::F {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(
        buffer.cursor_position().line,
        VIM_DEFAULT_PAGE_SCROLL_LINES * 3
    );
    assert!(pending.is_none());

    let page_up = handle_vim_editor_key_event(
        &mut buffer,
        Key::B,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(page_up.handled);
    assert!(!page_up.changed);
    assert_eq!(
        buffer.cursor_position().line,
        VIM_DEFAULT_PAGE_SCROLL_LINES * 2
    );
    assert!(pending.is_none());

    for key in [Key::Num2, Key::B] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::B {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_braces_move_between_paragraphs_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        "alpha\nbeta\n\n  \ngamma\n\n delta\nomega".to_owned(),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let next = handle_vim_editor_key_event(
        &mut buffer,
        Key::CloseBracket,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(next.suppress_text, Some('}'));
    assert_eq!(buffer.cursor_position().line, 4);
    assert_eq!(buffer.cursor_position().column, 0);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::CloseBracket] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::CloseBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 7);
    assert!(pending.is_none());

    let previous = handle_vim_editor_key_event(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(previous.handled);
    assert!(!previous.changed);
    assert_eq!(previous.suppress_text, Some('{'));
    assert_eq!(buffer.cursor_position().line, 6);
    assert_eq!(buffer.cursor_position().column, 0);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::OpenBracket] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::OpenBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_brace_operator_motions_delete_and_change_paragraphs() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha\n\nbeta\n\ncharlie\n\ndelta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::D, Key::Num2, Key::CloseBracket] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            if key == Key::CloseBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "charlie\n\ndelta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha\n\nbeta\n\n", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "one\n\nalpha\nbeta\n\nomega".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(5, 0));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for key in [Key::C, Key::OpenBracket] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            if key == Key::OpenBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nomega");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 0));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha\nbeta\n\n", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
}
