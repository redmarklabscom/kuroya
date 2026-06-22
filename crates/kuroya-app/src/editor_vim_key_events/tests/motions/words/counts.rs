use super::*;

#[test]
fn normal_mode_counts_repeat_motions_and_delete_forward() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let count = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let right = handle_vim_editor_key_event(
        &mut buffer,
        Key::L,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(count.handled);
    assert_eq!(count.suppress_text, Some('3'));
    assert!(right.handled);
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    let count = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num2,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let delete = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(count.handled);
    assert!(delete.changed);
    assert_eq!(buffer.text(), "alp beta gamma");
    assert!(pending.is_none());
}
