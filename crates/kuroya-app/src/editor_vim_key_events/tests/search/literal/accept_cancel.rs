use super::*;

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
