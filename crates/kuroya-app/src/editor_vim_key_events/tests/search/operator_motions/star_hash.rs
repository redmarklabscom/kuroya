use super::*;

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
