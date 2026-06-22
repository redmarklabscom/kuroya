use super::*;

#[test]
fn normal_mode_space_and_backspace_move_without_editing() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(2);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let space = handle_vim_editor_key_event(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(space.handled);
    assert!(!space.changed);
    assert_eq!(space.suppress_text, Some(' '));
    assert_eq!(buffer.cursor(), 3);

    for key in [Key::Num2, Key::Space] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), 5);

    let backspace = handle_vim_editor_key_event(
        &mut buffer,
        Key::Backspace,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(backspace.handled);
    assert!(!backspace.changed);
    assert_eq!(buffer.cursor(), 4);
    assert_eq!(buffer.text(), "abcdef");
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(" ".to_owned()),
            Event::Key {
                key: Key::Backspace,
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

#[test]
fn normal_mode_space_and_backspace_wrap_lines_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "ab\ncd\nef".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let space = handle_vim_editor_key_event(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(space.handled);
    assert!(!space.changed);
    assert_eq!(space.suppress_text, Some(' '));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Space] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 0));
    assert!(pending.is_none());

    let backspace = handle_vim_editor_key_event(
        &mut buffer,
        Key::Backspace,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(backspace.handled);
    assert!(!backspace.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 1));
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Backspace] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 1));
    assert_eq!(buffer.text(), "ab\ncd\nef");
    assert!(pending.is_none());
}
