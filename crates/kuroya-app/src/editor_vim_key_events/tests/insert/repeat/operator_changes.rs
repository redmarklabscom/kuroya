use super::*;

#[test]
fn normal_mode_period_repeats_delete_around_word_change() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::D, Key::A, Key::W, Key::Period] {
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
    assert_eq!(buffer.text(), "gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta ")
    );
    assert!(pending.is_none());
}

#[test]
fn normal_mode_period_repeats_change_around_word() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::C, Key::A, Key::W] {
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
    assert_eq!(mode, EditorVimMode::Insert);
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
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta ")
    );
}
