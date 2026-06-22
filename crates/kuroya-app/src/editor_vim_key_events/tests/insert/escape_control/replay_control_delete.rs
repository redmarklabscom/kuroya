use super::*;

#[test]
fn normal_mode_period_replays_insert_mode_ctrl_u_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "prefix-tail\nagain-more".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);

    buffer.insert_at_cursors("abc");
    vim_record_inserted_text(&mut last_change, "abc");
    let delete_line_backward = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(delete_line_backward.handled);
    assert!(delete_line_backward.changed);
    buffer.insert_at_cursors("X");
    vim_record_inserted_text(&mut last_change, "X");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "X-tail\nagain-more");

    buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "X-tail\nX-more");
}

#[test]
fn normal_mode_period_replays_insert_mode_ctrl_w_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "one two-tail\nred blue-more".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);

    let delete_word_backward = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::W,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(delete_word_backward.handled);
    assert!(delete_word_backward.changed);
    buffer.insert_at_cursors("new");
    vim_record_inserted_text(&mut last_change, "new");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "one new-tail\nred blue-more");

    buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "one new-tail\nred new-more");
}

#[test]
fn normal_mode_period_replays_insert_mode_ctrl_h_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "ab-cd\nxy-zw".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 2));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let insert = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(insert.handled);
    assert_eq!(mode, EditorVimMode::Insert);

    buffer.insert_at_cursors("q");
    vim_record_inserted_text(&mut last_change, "q");
    let delete_char_backward = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::H,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(delete_char_backward.handled);
    assert!(delete_char_backward.changed);
    buffer.insert_at_cursors("R");
    vim_record_inserted_text(&mut last_change, "R");

    let escape = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(escape.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "abR-cd\nxy-zw");

    buffer.set_single_cursor(buffer.line_column_to_char(1, 2));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "abR-cd\nxyR-zw");
}
