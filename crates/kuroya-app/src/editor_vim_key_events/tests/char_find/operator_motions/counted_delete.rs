use super::*;

#[test]
fn normal_mode_char_find_delete_motion_accepts_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abxcdxzz".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::D, Key::Num2, Key::F, Key::X] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "zz");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("abxcdx", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
}
