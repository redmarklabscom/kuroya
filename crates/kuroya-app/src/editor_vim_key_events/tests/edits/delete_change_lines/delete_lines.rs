use super::*;

#[test]
fn normal_mode_dd_deletes_current_line_after_pending_key() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let first = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let second = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(first.handled);
    assert!(!first.changed);
    assert!(second.changed);
    assert_eq!(buffer.text(), "one\nthree\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        Some(super::EditorVimPendingKey::DeleteLine(1)),
    ));
}

#[test]
fn normal_mode_counted_dd_deletes_multiple_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for key in [Key::Num2, Key::D, Key::D] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "one\nfour\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_dd_removes_line_break_without_trailing_newline() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for key in [Key::D, Key::D] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "one\nthree");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_dd_removes_last_content_line_break_with_trailing_newline() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for key in [Key::D, Key::D] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "one");
    assert!(pending.is_none());
}
