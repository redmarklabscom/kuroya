use super::*;

#[test]
fn normal_mode_ctrl_open_bracket_clears_pending_key() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = Some(EditorVimPendingKey::DeleteLine(1));

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}
