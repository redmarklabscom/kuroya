use super::*;

mod literal;

#[test]
fn vim_pending_search_status_label_names_direction_and_bounds_query() {
    VIM_SEARCH_INPUT.with(|input| *input.borrow_mut() = "needle".to_owned());
    assert_eq!(
        vim_pending_search_status_label(Some(EditorVimPendingKey::SearchInput {
            count: 1,
            forward: true,
        })),
        Some("/needle".to_owned())
    );
    assert_eq!(
        vim_pending_search_status_label(Some(EditorVimPendingKey::SearchInput {
            count: 1,
            forward: false,
        })),
        Some("?needle".to_owned())
    );
    assert_eq!(
        vim_pending_search_status_label(Some(EditorVimPendingKey::Go(None))),
        None
    );

    VIM_SEARCH_INPUT.with(|input| *input.borrow_mut() = "x".repeat(140));
    let label = vim_pending_search_status_label(Some(EditorVimPendingKey::SearchInput {
        count: 1,
        forward: true,
    }))
    .expect("pending search status label");

    assert!(label.starts_with('/'));
    assert!(label.ends_with("..."));
    assert!(label.chars().count() < 120);
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

#[test]
fn normal_mode_star_and_hash_search_word_under_cursor_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha alphabet alpha\nbeta alpha".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let next = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(next.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert_eq!(buffer.text(), "alpha alphabet alpha\nbeta alpha");
    assert!(pending.is_none());

    buffer.set_single_cursor(0);
    for (key, modifiers) in [(Key::Num2, Modifiers::NONE), (Key::Num8, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 5));
    assert!(pending.is_none());

    let previous = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(previous.handled);
    assert!(!previous.changed);
    assert_eq!(previous.suppress_text, Some('#'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
    for (key, modifiers) in [(Key::Num2, Modifiers::NONE), (Key::Num3, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), 0);
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
                key: Key::Num3,
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
fn normal_mode_g_star_searches_partial_word_matches_with_counts_and_repeat() {
    let mut buffer = TextBuffer::from_text(1901, None, "alpha alphabet alpha alphabet".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let count = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num2,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(count.handled);
    assert!(!count.changed);

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(go.handled);
    assert!(!go.changed);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(pending.is_some());

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
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
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
    assert_eq!(repeat.suppress_text, Some('n'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 21));

    let reverse = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(reverse.suppress_text, Some('N'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Num2,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::G,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Num8,
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
fn normal_mode_g_hash_searches_partial_word_matches_and_repeats_backward() {
    let mut buffer = TextBuffer::from_text(1902, None, "alpha alphabet alpha alphabet".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 15));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(go.handled);
    assert!(!go.changed);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(pending.is_some());

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
    assert_eq!(repeat.suppress_text, Some('n'));
    assert_eq!(buffer.cursor(), 0);

    let reverse = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(reverse.suppress_text, Some('N'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::G,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Num3,
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
fn normal_mode_star_search_uses_next_word_when_cursor_is_not_on_word() {
    let mut buffer = TextBuffer::from_text(92, None, "alpha, beta beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let from_punctuation = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_punctuation.handled);
    assert!(!from_punctuation.changed);
    assert_eq!(from_punctuation.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 12));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let from_space = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_space.handled);
    assert!(!from_space.changed);
    assert_eq!(from_space.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 12));
    assert!(pending.is_none());
}

#[test]
fn normal_mode_hash_search_uses_next_word_then_searches_backward() {
    let mut buffer = TextBuffer::from_text(93, None, "beta alpha, beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 10));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let from_punctuation = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_punctuation.handled);
    assert!(!from_punctuation.changed);
    assert_eq!(from_punctuation.suppress_text, Some('#'));
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_star_and_hash_use_previous_word_at_line_end() {
    let mut buffer = TextBuffer::from_text(94, None, "alpha beta\nbeta alpha".to_owned());
    buffer.set_single_cursor(buffer.line_content_end_char(0));
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
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_content_end_char(1));
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
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_star_at_line_end_still_skips_punctuation_only_suffix() {
    let mut buffer = TextBuffer::from_text(95, None, "alpha, beta beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let from_punctuation = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_punctuation.handled);
    assert!(!from_punctuation.changed);
    assert_eq!(from_punctuation.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 12));
    assert!(pending.is_none());
}

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

#[test]
fn normal_mode_n_and_shift_n_work_as_operator_search_motions() {
    let mut buffer = TextBuffer::from_text(1903, None, "alpha beta alpha beta alpha".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let star = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(star.handled);
    assert!(!star.changed);
    assert!(pending.is_none());
    buffer.set_single_cursor(0);

    for key in [Key::D, Key::N] {
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
    assert_eq!(buffer.text(), "alpha beta alpha");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha beta ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1904, None, "alpha beta alpha beta alpha".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    let star = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(star.handled);
    assert!(!star.changed);
    buffer.set_single_cursor(0);

    for key in [Key::D, Key::Num2, Key::N] {
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
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some((
            "alpha beta alpha beta ",
            EditorVimRegisterKind::Characterwise
        ))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1905, None, "alpha beta alpha beta alpha".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    let star = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(star.handled);
    assert!(!star.changed);
    buffer.set_single_cursor(buffer.line_column_to_char(0, 22));

    for (key, modifiers) in [(Key::C, Modifiers::NONE), (Key::N, Modifiers::SHIFT)] {
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha beta alpha");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha beta ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::D, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE)
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::C, Modifiers::NONE),
            key_event(Key::N, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_gn_and_g_shift_n_work_as_operator_search_match_motions() {
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1959, None, "alpha beta gamma beta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    vim_set_last_search(&buffer, "beta", true, false);
    for (key, modifiers) in [
        (Key::D, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::N, Modifiers::NONE),
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
    assert_eq!(buffer.text(), "alpha  gamma beta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1960, None, "alpha beta gamma beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 17));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;
    vim_set_last_search(&buffer, "beta", true, false);

    for (key, modifiers) in [
        (Key::Y, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::N, Modifiers::SHIFT),
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

    assert_eq!(buffer.text(), "alpha beta gamma beta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1961, None, "alpha beta gamma beta".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;
    vim_set_last_search(&buffer, "beta", true, false);

    for (key, modifiers) in [
        (Key::C, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::N, Modifiers::NONE),
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha  gamma beta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    assert!(vim_events_include_mutation(
        &[
            key_event(Key::D, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::C, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Y, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::N, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}

#[test]
fn normal_mode_gn_and_g_shift_n_enter_visual_character_search_match() {
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1965, None, "alpha beta gamma beta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    vim_set_last_search(&buffer, "beta", true, false);
    for (key, modifiers) in [(Key::G, Modifiers::NONE), (Key::N, Modifiers::NONE)] {
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
        assert!(!result.changed);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.selected_text().as_deref(), Some("beta"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 6,
            cursor: 9,
        })
    );

    pending = None;
    buffer.set_single_cursor(buffer.line_column_to_char(0, 16));
    for (key, modifiers) in [(Key::G, Modifiers::NONE), (Key::N, Modifiers::SHIFT)] {
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
        assert!(!result.changed);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("beta"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 6,
            cursor: 9,
        })
    );
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE)
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}

#[test]
fn normal_mode_gn_operator_uses_current_search_match() {
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1966, None, "alpha beta gamma beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    vim_set_last_search(&buffer, "beta", true, false);
    for (key, modifiers) in [
        (Key::C, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::N, Modifiers::NONE),
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha  gamma beta");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}

#[test]
fn normal_mode_star_and_hash_work_as_operator_search_motions() {
    let mut buffer = TextBuffer::from_text(1906, None, "alpha beta alpha beta alpha".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::Num8, Modifiers::SHIFT)] {
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
    assert_eq!(buffer.text(), "alpha beta alpha");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha beta ", EditorVimRegisterKind::Characterwise))
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
    assert_eq!(buffer.text(), "alpha");
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1907, None, "alpha beta alpha beta".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for (key, modifiers) in [(Key::Y, Modifiers::NONE), (Key::Num8, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "alpha beta alpha beta");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha beta ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    let next = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));

    buffer = TextBuffer::from_text(1908, None, "alpha beta alpha beta alpha".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 22));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for (key, modifiers) in [(Key::C, Modifiers::NONE), (Key::Num3, Modifiers::SHIFT)] {
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha beta alpha");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha beta ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::D, Modifiers::NONE),
            key_event(Key::Num8, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Y, Modifiers::NONE),
            key_event(Key::Num8, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_g_star_and_g_hash_work_as_operator_search_motions() {
    let mut buffer = TextBuffer::from_text(1954, None, "alpha alphabet alpha alphabet".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::D, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::Num8, Modifiers::SHIFT),
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
    assert_eq!(buffer.text(), "alphabet alpha alphabet");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha ", EditorVimRegisterKind::Characterwise))
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
    assert_eq!(buffer.text(), "alphabet");
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1955, None, "alpha alphabet alpha".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 15));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::Y, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::Num3, Modifiers::SHIFT),
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

    assert_eq!(buffer.text(), "alpha alphabet alpha");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alphabet ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1956, None, "alpha alphabet alpha".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 15));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::C, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::Num3, Modifiers::SHIFT),
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha alpha");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alphabet ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::D, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::Num8, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Y, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::Num3, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
