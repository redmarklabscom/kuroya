use super::*;

#[test]
fn normal_mode_char_find_scans_by_buffer_indices_on_current_line() {
    let mut buffer = TextBuffer::from_text(1, None, "a\u{e9}x\n\u{e9}x".to_owned());

    buffer.set_single_cursor(0);
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::FindForward,
        'x'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 2));

    assert!(!vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::FindForward,
        'x'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 2));

    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::FindBackward,
        '\u{e9}'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));

    buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::TillForward,
        'x'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 1));

    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    assert!(vim_apply_char_find(
        &mut buffer,
        1,
        EditorVimCharFindMotion::TillBackward,
        '\u{e9}'
    ));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 1));
}
