use super::*;

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
