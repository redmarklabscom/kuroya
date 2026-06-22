use super::*;

#[test]
fn normal_mode_plus_minus_enter_and_underscore_move_to_first_non_whitespace() {
    let mut buffer = TextBuffer::from_text(1, None, "root\n    child\n\tleaf\nlast\n".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let plus = handle_vim_editor_key_event(
        &mut buffer,
        Key::Equals,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(plus.handled);
    assert!(!plus.changed);
    assert_eq!(plus.suppress_text, Some('+'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::Equals, Modifiers::SHIFT),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 1));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let enter = handle_vim_editor_key_event(
        &mut buffer,
        Key::Enter,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(enter.handled);
    assert!(!enter.changed);
    assert_eq!(enter.suppress_text, None);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(2, 3));
    let minus = handle_vim_editor_key_event(
        &mut buffer,
        Key::Minus,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(minus.handled);
    assert!(!minus.changed);
    assert_eq!(minus.suppress_text, Some('-'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 4));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let underscore = handle_vim_editor_key_event(
        &mut buffer,
        Key::Minus,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(underscore.handled);
    assert!(!underscore.changed);
    assert_eq!(underscore.suppress_text, Some('_'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 0));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    for (key, modifiers) in [(Key::Num3, Modifiers::NONE), (Key::Minus, Modifiers::SHIFT)] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 1));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Equals,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Text("+".to_owned()),
            Event::Key {
                key: Key::Minus,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("-".to_owned()),
            Event::Key {
                key: Key::Minus,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Text("_".to_owned()),
            Event::Key {
                key: Key::Enter,
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
