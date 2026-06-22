use super::*;

#[test]
fn normal_mode_n_and_shift_n_repeat_last_word_search() {
    let mut buffer = TextBuffer::from_text(91, None, "alpha beta alpha beta alpha".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let star = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(star.handled);
    assert!(!star.changed);
    assert_eq!(star.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let repeat = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(repeat.suppress_text, Some('n'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert!(pending.is_none());

    let next = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 22));

    let previous = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(previous.handled);
    assert!(!previous.changed);
    assert_eq!(previous.suppress_text, Some('N'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));

    for key in [Key::Num2, Key::N] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 22));
    let hash = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(hash.handled);
    assert!(!hash.changed);
    assert_eq!(hash.suppress_text, Some('#'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));

    let repeat_backward = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(repeat_backward.handled);
    assert!(!repeat_backward.changed);
    assert_eq!(buffer.cursor(), 0);

    let reverse_to_forward = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(reverse_to_forward.handled);
    assert!(!reverse_to_forward.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Num8,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Key {
                key: Key::N,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::N,
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
fn vim_word_search_target_preserves_forward_backward_wrap_counts() {
    let buffer = TextBuffer::from_text(
        1904,
        None,
        "alpha beta alpha beta alpha beta alpha".to_owned(),
    );

    assert_eq!(
        vim_search_word_target(&buffer, "alpha", 0, 1, true, true),
        Some(buffer.line_column_to_char(0, 11))
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alpha", 0, 4, true, true),
        Some(0)
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alpha", 0, 7, true, true),
        Some(buffer.line_column_to_char(0, 33))
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alpha", 22, 2, false, true),
        Some(0)
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alpha", 0, 1, false, true),
        Some(buffer.line_column_to_char(0, 33))
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alpha", 0, 6, false, true),
        Some(buffer.line_column_to_char(0, 22))
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alp", 0, 2, true, false),
        Some(buffer.line_column_to_char(0, 22))
    );
    assert_eq!(
        vim_search_word_target(&buffer, "alphabet", 0, 1, true, true),
        None
    );
}

#[test]
fn vim_set_last_search_reuses_existing_buffer_entry() {
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let buffer = TextBuffer::from_text(1905, None, "alphabet alpha beta".to_owned());

    vim_set_last_search(&buffer, "alphabet", true, true);
    let initial_capacity = VIM_SEARCHES.with(|searches| {
        let searches = searches.borrow();
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].buffer_id, buffer.id());
        assert_eq!(searches[0].search.word, "alphabet");
        assert!(searches[0].search.forward);
        assert!(searches[0].search.whole_word);
        searches[0].search.word.capacity()
    });

    vim_set_last_search(&buffer, "alphabet", true, true);
    VIM_SEARCHES.with(|searches| {
        let searches = searches.borrow();
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].search.word, "alphabet");
        assert_eq!(searches[0].search.word.capacity(), initial_capacity);
    });

    vim_set_last_search(&buffer, "alpha", false, false);
    VIM_SEARCHES.with(|searches| {
        let searches = searches.borrow();
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].search.word, "alpha");
        assert!(!searches[0].search.forward);
        assert!(!searches[0].search.whole_word);
        assert_eq!(searches[0].search.word.capacity(), initial_capacity);
    });
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}
