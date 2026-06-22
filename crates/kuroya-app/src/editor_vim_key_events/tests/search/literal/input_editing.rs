use super::*;

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
