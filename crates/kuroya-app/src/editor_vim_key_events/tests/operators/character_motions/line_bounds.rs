use super::*;

#[test]
fn normal_mode_d_and_c_line_bound_motions_use_characterwise_ranges() {
    let mut buffer = TextBuffer::from_text(1, None, "  alpha beta\nnext\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 4));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::Num4, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "  al\nnext\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("pha beta")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(4);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::C, Key::Num0] {
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
    assert_eq!(buffer.text(), "ef");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("abcd")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "    alpha\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 8));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::Num6, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "    a\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alph")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "one\ntwo words\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::D, Modifiers::NONE),
        (Key::Num4, Modifiers::SHIFT),
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

    assert_eq!(buffer.text(), "o\nthree\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("ne\ntwo words")
    );
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
                key: Key::Num4,
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
                key: Key::Num0,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
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
                key: Key::Num6,
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
