use super::*;

#[test]
fn normal_mode_gu_and_g_shift_u_operator_motions_convert_case_with_counts_and_repeat() {
    let mut buffer = TextBuffer::from_text(1, None, "ALPHA BETA GAMMA".to_owned());
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
        (Key::U, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
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

    assert_eq!(buffer.text(), "alpha beta GAMMA");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("kept", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
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
    assert_eq!(buffer.text(), "alpha beta gamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));

    buffer = TextBuffer::from_text(1, None, "alpha beta gamma delta".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::U, Modifiers::SHIFT),
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

    assert_eq!(buffer.text(), "ALPHA BETA gamma delta");
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
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
    assert_eq!(buffer.text(), "ALPHA BETA GAMMA DELTA");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
}

#[test]
fn normal_mode_gu_char_find_motion_converts_case_with_counts_and_repeat() {
    let mut buffer = TextBuffer::from_text(1, None, "ABX CDX EFX GHX".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::G, Modifiers::NONE),
        (Key::U, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
        (Key::F, Modifiers::NONE),
        (Key::X, Modifiers::SHIFT),
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

    assert_eq!(buffer.text(), "abx cdx EFX GHX");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        last_char_find,
        Some(super::EditorVimCharFind {
            motion: EditorVimCharFindMotion::FindForward,
            target: 'X',
        })
    );
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 8));
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
    assert_eq!(buffer.text(), "abx cdx efx ghx");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 8));
}

#[test]
fn normal_mode_gu_and_g_shift_u_mutation_detection_waits_for_motion() {
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::NONE),
            key_event(Key::W, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
            key_event(Key::F, Modifiers::NONE),
            key_event(Key::X, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::NONE),
            key_event(Key::U, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
