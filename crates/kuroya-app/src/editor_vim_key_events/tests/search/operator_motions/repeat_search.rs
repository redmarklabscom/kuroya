use super::*;

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
