use super::*;

#[test]
fn normal_mode_star_and_hash_search_word_under_cursor_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha alphabet alpha\nbeta alpha".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let next = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(next.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert_eq!(buffer.text(), "alpha alphabet alpha\nbeta alpha");
    assert!(pending.is_none());

    buffer.set_single_cursor(0);
    for (key, modifiers) in [(Key::Num2, Modifiers::NONE), (Key::Num8, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 5));
    assert!(pending.is_none());

    let previous = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(previous.handled);
    assert!(!previous.changed);
    assert_eq!(previous.suppress_text, Some('#'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
    for (key, modifiers) in [(Key::Num2, Modifiers::NONE), (Key::Num3, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Num8,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Key {
                key: Key::Num3,
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
