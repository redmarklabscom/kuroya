use super::*;

#[test]
fn normal_mode_open_line_commands_collapse_selection_before_insert() {
    let mut buffer = TextBuffer::from_text(1, None, "  one\n  two".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    buffer.set_selection(
        buffer.line_column_to_char(0, 0),
        buffer.line_column_to_char(0, 5),
    );

    let open_below = handle_vim_editor_key_event(
        &mut buffer,
        Key::O,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(open_below.handled);
    assert!(open_below.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert!(!buffer.has_selection());
    assert_eq!(buffer.text(), "  one\n  \n  two");
    assert_eq!(buffer.selections(), &[Selection::caret(8)]);

    buffer = TextBuffer::from_text(1, None, "  one\n  two".to_owned());
    mode = EditorVimMode::Normal;
    pending = None;
    buffer.set_selection(
        buffer.line_column_to_char(1, 0),
        buffer.line_column_to_char(1, 5),
    );

    let open_above = handle_vim_editor_key_event(
        &mut buffer,
        Key::O,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(open_above.handled);
    assert!(open_above.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert!(!buffer.has_selection());
    assert_eq!(buffer.text(), "  one\n  \n  two");
    assert_eq!(buffer.selections(), &[Selection::caret(8)]);
}

#[test]
fn normal_mode_o_and_shift_o_open_indented_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "  one\n  two\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 3));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    assert_eq!(vim_open_line_below_text("\t  "), "\n\t  ");
    assert_eq!(vim_open_line_above_text("\t  "), "\t  \n");

    let open_below = handle_vim_editor_key_event(
        &mut buffer,
        Key::O,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(open_below.handled);
    assert!(open_below.changed);
    assert_eq!(open_below.suppress_text, Some('o'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "  one\n  \n  two\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 2));

    buffer = TextBuffer::from_text(1, None, "  one\n  two\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 3));
    mode = EditorVimMode::Normal;
    pending = None;

    let open_above = handle_vim_editor_key_event(
        &mut buffer,
        Key::O,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(open_above.handled);
    assert!(open_above.changed);
    assert_eq!(open_above.suppress_text, Some('O'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "  one\n  \n  two\n");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 2));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::O,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::O,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));
}
