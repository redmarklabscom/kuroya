use super::*;

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
