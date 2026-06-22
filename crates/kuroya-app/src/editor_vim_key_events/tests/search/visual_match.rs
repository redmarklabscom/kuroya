use super::*;

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
