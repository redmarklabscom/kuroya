use super::*;

#[test]
fn normal_mode_delete_lines_updates_linewise_register_for_put() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 0));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::D, Key::D] {
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

    assert_eq!(buffer.text(), "one\nthree");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("two\n")
    );

    buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
    let put = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::P,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(put.handled);
    assert!(put.changed);
    assert_eq!(buffer.text(), "one\ntwo\nthree");
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());
}
