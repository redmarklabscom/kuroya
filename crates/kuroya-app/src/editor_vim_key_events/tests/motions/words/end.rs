use super::*;

#[test]
fn normal_mode_e_moves_to_word_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(end.handled);
    assert_eq!(end.suppress_text, Some('e'));
    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::E] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 10);
    assert!(pending.is_none());
}
