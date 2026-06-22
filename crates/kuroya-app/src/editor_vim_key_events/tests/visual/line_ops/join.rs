use super::*;

#[test]
fn normal_mode_visual_character_shift_j_joins_selected_lines_and_repeats() {
    let mut buffer =
        TextBuffer::from_text(1, None, "one\n  two\nthree\nfour\nfive\nsix\n".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::J, Modifiers::NONE),
        (Key::J, Modifiers::NONE),
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

    let join = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::J,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(join.handled);
    assert!(join.changed);
    assert_eq!(join.suppress_text, Some('J'));
    assert_eq!(buffer.text(), "one two three\nfour\nfive\nsix\n");
    assert_eq!(buffer.cursor(), 0);
    assert!(!buffer.has_selection());
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert!(last_change.is_some());

    buffer.set_single_cursor(buffer.line_column_to_char(1, 0));
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
    assert_eq!(buffer.text(), "one two three\nfour five six\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::J, Modifiers::NONE),
            key_event(Key::J, Modifiers::NONE),
            key_event(Key::J, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
