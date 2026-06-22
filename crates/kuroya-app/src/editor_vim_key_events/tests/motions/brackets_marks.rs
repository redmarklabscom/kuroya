use super::*;

#[test]
fn normal_mode_percent_moves_and_operates_on_matching_brackets() {
    let mut buffer = TextBuffer::from_text(1, None, "(alpha [beta]) tail".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    let forward = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num5,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(forward.handled);
    assert!(!forward.changed);
    assert_eq!(forward.suppress_text, Some('%'));
    assert_eq!(buffer.cursor(), 13);

    let backward = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num5,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(backward.handled);
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());

    buffer.set_single_cursor(7);
    for (key, modifiers) in [(Key::D, Modifiers::NONE), (Key::Num5, Modifiers::SHIFT)] {
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

    assert_eq!(buffer.text(), "(alpha ) tail");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("[beta]")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "(alpha [beta]) tail".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [(Key::C, Modifiers::NONE), (Key::Num5, Modifiers::SHIFT)] {
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
    assert_eq!(buffer.text(), " tail");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("(alpha [beta])")
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
                key: Key::Num5,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(!vim_events_include_mutation(
        &[Event::Key {
            key: Key::Num5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_marks_set_and_jump_linewise_or_exact() {
    let mut buffer = TextBuffer::from_text(1, None, "zero\n    alpha beta\nthree\n".to_owned());
    let marked_cursor = buffer.line_column_to_char(1, 10);
    buffer.set_single_cursor(marked_cursor);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let mark = handle_vim_editor_key_event(
        &mut buffer,
        Key::M,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(mark.handled);
    assert_eq!(mark.suppress_text, Some('m'));
    let mark_name = handle_vim_editor_key_event(
        &mut buffer,
        Key::A,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(mark_name.handled);
    assert_eq!(mark_name.suppress_text, Some('a'));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for key in [Key::Quote, Key::A] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for key in [Key::Backtick, Key::A] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), marked_cursor);
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::M,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Quote,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
