use super::*;

#[test]
fn normal_mode_braces_move_between_paragraphs_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        "alpha\nbeta\n\n  \ngamma\n\n delta\nomega".to_owned(),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let next = handle_vim_editor_key_event(
        &mut buffer,
        Key::CloseBracket,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(next.suppress_text, Some('}'));
    assert_eq!(buffer.cursor_position().line, 4);
    assert_eq!(buffer.cursor_position().column, 0);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::CloseBracket] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::CloseBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 7);
    assert!(pending.is_none());

    let previous = handle_vim_editor_key_event(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(previous.handled);
    assert!(!previous.changed);
    assert_eq!(previous.suppress_text, Some('{'));
    assert_eq!(buffer.cursor_position().line, 6);
    assert_eq!(buffer.cursor_position().column, 0);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::OpenBracket] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::OpenBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 0);
    assert!(pending.is_none());
}
