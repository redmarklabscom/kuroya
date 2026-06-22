use super::*;

#[test]
fn normal_mode_big_word_motions_and_operator_forms_use_whitespace_groups() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    let big_word = handle_vim_editor_key_event(
        &mut buffer,
        Key::W,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(big_word.handled);
    assert_eq!(big_word.suppress_text, Some('W'));
    assert_eq!(buffer.cursor(), 6);

    buffer.set_single_cursor(0);
    for (key, modifiers) in [(Key::Num2, Modifiers::NONE), (Key::W, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor(), 18);

    buffer.set_single_cursor(0);
    let big_end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(big_end.handled);
    assert_eq!(big_end.suppress_text, Some('E'));
    assert_eq!(buffer.cursor(), 4);

    buffer.set_single_cursor(buffer.len_chars());
    let big_back = handle_vim_editor_key_event(
        &mut buffer,
        Key::B,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(big_back.handled);
    assert_eq!(big_back.suppress_text, Some('B'));
    assert_eq!(buffer.cursor(), 18);
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(0);
    mode = EditorVimMode::Normal;
    pending = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::W, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "beta.gamma  delta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alpha ")
    );

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::B, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "alpha gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta.")
    );

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::C, Modifiers::NONE), (Key::E, Modifiers::SHIFT)] {
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

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), " beta.gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alpha")
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::D,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::W,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::E,
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
