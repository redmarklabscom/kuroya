use super::*;

#[test]
fn normal_mode_hjkl_moves_without_editing() {
    let mut buffer = TextBuffer::from_text(1, None, "abc\ndef\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let right = handle_vim_editor_key_event(
        &mut buffer,
        Key::L,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::J,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(right.handled);
    assert!(down.handled);
    assert!(!right.changed);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, 1);
    assert_eq!(buffer.cursor_position().column, 2);
    assert_eq!(buffer.text(), "abc\ndef\n");
}
