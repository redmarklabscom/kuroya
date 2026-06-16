use super::*;

#[test]
fn normal_mode_slash_literal_search_accepts_query_and_repeats() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1956, None, "alpha beta gamma beta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers, suppressed) in [
        (Key::Slash, Modifiers::NONE, Some('/')),
        (Key::B, Modifiers::NONE, Some('b')),
        (Key::E, Modifiers::NONE, Some('e')),
        (Key::T, Modifiers::NONE, Some('t')),
        (Key::A, Modifiers::NONE, Some('a')),
        (Key::Enter, Modifiers::NONE, None),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
        assert_eq!(result.suppress_text, suppressed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(pending.is_none());

    let repeat = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 17));

    let reverse = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
            key_event(Key::T, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE),
            key_event(Key::N, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_literal_search_input_ctrl_w_deletes_previous_query_word() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer =
        TextBuffer::from_text(1957, None, "zero alpha gamma tail alpha beta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers) in [
        (Key::Slash, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::P, Modifiers::NONE),
        (Key::H, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::Space, Modifiers::NONE),
        (Key::B, Modifiers::NONE),
        (Key::E, Modifiers::NONE),
        (Key::T, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    let delete_word = handle_vim_editor_key_event(
        &mut buffer,
        Key::W,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );
    assert!(delete_word.handled);
    assert!(!delete_word.changed);
    assert!(matches!(
        pending,
        Some(EditorVimPendingKey::SearchInput { .. })
    ));

    for (key, modifiers) in [
        (Key::G, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::M, Modifiers::NONE),
        (Key::M, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::Enter, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 5));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::P, Modifiers::NONE),
            key_event(Key::H, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Space, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
            key_event(Key::T, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::W, Modifiers::CTRL),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::M, Modifiers::NONE),
            key_event(Key::M, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_literal_search_input_ctrl_h_deletes_previous_query_char() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1958, None, "alpha beta gamma".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers) in [
        (Key::Slash, Modifiers::NONE),
        (Key::B, Modifiers::NONE),
        (Key::E, Modifiers::NONE),
        (Key::T, Modifiers::NONE),
        (Key::X, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    let delete_char = handle_vim_editor_key_event(
        &mut buffer,
        Key::H,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );
    assert!(delete_char.handled);
    assert!(!delete_char.changed);
    assert!(matches!(
        pending,
        Some(EditorVimPendingKey::SearchInput { .. })
    ));

    for (key, modifiers) in [(Key::A, Modifiers::NONE), (Key::Enter, Modifiers::NONE)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
            key_event(Key::T, Modifiers::NONE),
            key_event(Key::X, Modifiers::NONE),
            key_event(Key::H, Modifiers::CTRL),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_literal_search_input_ctrl_c_cancels_query() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1963, None, "alpha beta gamma".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers) in [
        (Key::Slash, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::M, Modifiers::NONE),
        (Key::M, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    let cancel = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );
    assert!(cancel.handled);
    assert!(!cancel.changed);
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
    VIM_SEARCH_INPUT.with(|input| assert!(input.borrow().is_empty()));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::M, Modifiers::NONE),
            key_event(Key::M, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::C, Modifiers::CTRL),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_literal_search_input_ctrl_m_and_ctrl_j_accept_query() {
    for (search_key, accept_key, start_column, expected_column) in [
        (Key::Slash, Key::M, 0, 6),
        (Key::Questionmark, Key::J, 21, 17),
    ] {
        VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
        let mut buffer = TextBuffer::from_text(1964, None, "alpha beta gamma beta".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, start_column));
        let mut mode = EditorVimMode::Normal;
        let mut pending = None;

        for (key, modifiers) in [
            (search_key, Modifiers::NONE),
            (Key::B, Modifiers::NONE),
            (Key::E, Modifiers::NONE),
            (Key::T, Modifiers::NONE),
            (Key::A, Modifiers::NONE),
        ] {
            let result =
                handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
            assert!(result.handled);
            assert!(!result.changed);
        }

        let accept = handle_vim_editor_key_event(
            &mut buffer,
            accept_key,
            Modifiers::CTRL,
            &mut mode,
            &mut pending,
        );
        assert!(accept.handled);
        assert!(!accept.changed);
        assert_eq!(
            buffer.cursor(),
            buffer.line_column_to_char(0, expected_column)
        );
        assert!(pending.is_none());
        assert!(!vim_events_include_mutation(
            &[
                key_event(search_key, Modifiers::NONE),
                key_event(Key::B, Modifiers::NONE),
                key_event(Key::E, Modifiers::NONE),
                key_event(Key::T, Modifiers::NONE),
                key_event(Key::A, Modifiers::NONE),
                key_event(accept_key, Modifiers::CTRL),
            ],
            EditorVimMode::Normal,
            None,
        ));
    }
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_literal_search_input_ctrl_u_clears_query() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1959, None, "alpha beta alpha beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 20));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers) in [
        (Key::Questionmark, Modifiers::NONE),
        (Key::B, Modifiers::NONE),
        (Key::E, Modifiers::NONE),
        (Key::T, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    let clear_query = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );
    assert!(clear_query.handled);
    assert!(!clear_query.changed);
    assert!(matches!(
        pending,
        Some(EditorVimPendingKey::SearchInput { .. })
    ));

    for (key, modifiers) in [
        (Key::A, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::P, Modifiers::NONE),
        (Key::H, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::Enter, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Questionmark, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
            key_event(Key::T, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::U, Modifiers::CTRL),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::P, Modifiers::NONE),
            key_event(Key::H, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_question_literal_search_supports_count_backspace_and_cancel() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1960, None, "beta alpha beta alpha beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 22));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::Slash, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::B, Modifiers::NONE),
        (Key::Backspace, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::P, Modifiers::NONE),
        (Key::H, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::Enter, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 5));
    assert!(pending.is_none());

    let cancel = handle_vim_editor_key_event(
        &mut buffer,
        Key::Questionmark,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(cancel.handled);
    assert_eq!(cancel.suppress_text, Some('?'));
    assert!(pending.is_some());

    let typed = handle_vim_editor_key_event(
        &mut buffer,
        Key::B,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(typed.handled);
    assert_eq!(typed.suppress_text, Some('b'));

    let escape = handle_vim_editor_key_event(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(escape.handled);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 5));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::Slash, Modifiers::SHIFT),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::Backspace, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::P, Modifiers::NONE),
            key_event(Key::H, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
            key_event(Key::Questionmark, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::Escape, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_empty_literal_search_reuses_last_pattern_in_requested_direction() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let mut buffer =
        TextBuffer::from_text(1962, None, "alpha beta alpha beta alpha beta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    vim_set_last_search(&buffer, "alpha", true, true);
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::Slash, Modifiers::NONE),
        (Key::Enter, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));
    assert!(pending.is_none());

    let repeat_forward = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(repeat_forward.handled);
    assert!(!repeat_forward.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));

    for (key, modifiers) in [
        (Key::Slash, Modifiers::SHIFT),
        (Key::Enter, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));

    let repeat_backward = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(repeat_backward.handled);
    assert!(!repeat_backward.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 22));

    let reverse_to_forward = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse_to_forward.handled);
    assert!(!reverse_to_forward.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));

    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
            key_event(Key::Slash, Modifiers::SHIFT),
            key_event(Key::Enter, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE),
            key_event(Key::N, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}
