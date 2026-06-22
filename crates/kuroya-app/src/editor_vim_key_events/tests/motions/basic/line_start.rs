use super::*;

#[test]
fn normal_mode_zero_stays_line_start_unless_count_is_active() {
    let mut buffer = TextBuffer::from_text(1, None, "zero\none\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(2, 2));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let zero = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num0,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(zero.handled);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 0));

    for key in [Key::Num1, Key::Num0, Key::K] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor_position().line, 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_line_start_keys_use_vim_semantics() {
    let mut buffer = TextBuffer::from_text(1, None, "    let value = 1;".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let caret = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num6,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(caret.handled);
    assert_eq!(caret.suppress_text, Some('^'));
    assert_eq!(buffer.cursor_position().column, 4);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
    let zero = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num0,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(zero.handled);
    assert_eq!(zero.suppress_text, Some('0'));
    assert_eq!(buffer.cursor_position().column, 0);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 12));
    let insert = handle_vim_editor_key_event(
        &mut buffer,
        Key::I,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(insert.handled);
    assert_eq!(insert.suppress_text, Some('I'));
    assert_eq!(buffer.cursor_position().column, 4);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
}
