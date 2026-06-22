use super::*;

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
