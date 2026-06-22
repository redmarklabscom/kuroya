use super::*;

#[test]
fn normal_mode_case_conversion_text_objects_convert_with_repeat() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = Some(EditorVimRegister {
        text: "kept".to_owned(),
        kind: EditorVimRegisterKind::Characterwise,
    });
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::G, Modifiers::NONE),
        (Key::U, Modifiers::SHIFT),
        (Key::I, Modifiers::NONE),
        (Key::W, Modifiers::NONE),
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

    assert_eq!(buffer.text(), "alpha BETA gamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("kept", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
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
    assert_eq!(buffer.text(), "alpha BETA GAMMA");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    pending = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::G, Modifiers::NONE),
        (Key::Backtick, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::W, Modifiers::NONE),
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

    assert_eq!(buffer.text(), "alpha BETA gamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(pending.is_none());
}

#[test]
fn normal_mode_case_conversion_text_object_mutation_detection_waits_for_kind() {
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
            key_event(Key::I, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
            key_event(Key::I, Modifiers::NONE),
            key_event(Key::W, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::Backtick, Modifiers::SHIFT),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::W, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
