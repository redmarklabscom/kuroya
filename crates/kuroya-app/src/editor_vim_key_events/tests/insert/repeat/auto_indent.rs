use super::*;

#[test]
fn normal_mode_period_replays_auto_indented_insert_enter() {
    let mut buffer = TextBuffer::from_text(1, None, "if ready {".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let append_line_end = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::A,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    assert!(append_line_end.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    buffer.insert_newline_with_indent_unit("  ");
    vim_record_insert_replay_key_with_auto_indent(
        &mut last_change,
        Key::Enter,
        Modifiers::NONE,
        true,
    );
    buffer.insert_at_cursors("x");
    vim_record_inserted_text(&mut last_change, "x");

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
    assert_eq!(buffer.text(), "if ready {\n  x\n  x");
}
