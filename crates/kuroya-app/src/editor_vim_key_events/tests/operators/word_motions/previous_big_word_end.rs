use super::*;

#[test]
fn normal_mode_g_shift_e_operator_motion_uses_previous_big_word_end() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for (key, modifiers) in [
        (Key::D, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::E, Modifiers::SHIFT),
    ] {
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
    }

    assert_eq!(buffer.text(), "alpha beta.gamm");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("a  delta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    mode = EditorVimMode::Normal;
    pending = None;

    for (key, modifiers) in [
        (Key::G, Modifiers::NONE),
        (Key::U, Modifiers::SHIFT),
        (Key::Num2, Modifiers::NONE),
        (Key::G, Modifiers::NONE),
        (Key::E, Modifiers::NONE),
    ] {
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
    }

    assert_eq!(buffer.text(), "alpha beta.GAMMA");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 10));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
            key_event(Key::G, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::U, Modifiers::SHIFT),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
