use super::*;

#[test]
fn normal_mode_shift_c_changes_to_line_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta\ngamma delta\nomega\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let change = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(change.handled);
    assert!(change.changed);
    assert_eq!(change.suppress_text, Some('C'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha \ngamma delta\nomega\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "alpha beta\ngamma delta\nomega\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::C] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::C {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha \nomega\n");
    assert!(pending.is_none());
}
