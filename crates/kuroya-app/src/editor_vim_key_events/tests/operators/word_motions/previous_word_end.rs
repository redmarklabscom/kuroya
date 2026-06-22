use super::*;

#[test]
fn normal_mode_ge_operator_motions_use_previous_word_end() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::D, Key::Num2, Key::G, Key::E] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "alpha beta");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 10));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some((".gamma", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::Y, Key::Num2, Key::G, Key::E] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.text(), "alpha beta.gamma");
    assert_eq!(buffer.cursor(), buffer.len_chars());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some((".gamma", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::C, Key::G, Key::E] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha beta.gamm");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("a", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::D, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE)
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::D, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Y, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
