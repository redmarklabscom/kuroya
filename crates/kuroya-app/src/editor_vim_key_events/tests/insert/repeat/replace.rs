use super::*;

#[test]
fn normal_mode_period_replays_replace_change() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::R, Modifiers::NONE),
        (Key::X, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::Period, Modifiers::NONE),
    ] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "xxc");
}
