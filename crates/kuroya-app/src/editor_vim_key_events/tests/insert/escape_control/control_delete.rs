use super::*;

#[test]
fn insert_mode_ctrl_h_deletes_previous_char() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(2);
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::H,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "ac");
    assert_eq!(buffer.cursor(), 1);
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::H,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        EditorVimMode::Insert,
        None,
    ));
}

#[test]
fn insert_mode_ctrl_u_deletes_to_line_start() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha\nbeta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "alpha\ngamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::U,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        EditorVimMode::Insert,
        None,
    ));

    let no_op = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(no_op.handled);
    assert!(!no_op.changed);
    assert_eq!(buffer.text(), "alpha\ngamma");
}

#[test]
fn insert_mode_ctrl_w_deletes_previous_word() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::W,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(buffer.text(), "alpha gamma");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::W,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
        EditorVimMode::Insert,
        None,
    ));
}
