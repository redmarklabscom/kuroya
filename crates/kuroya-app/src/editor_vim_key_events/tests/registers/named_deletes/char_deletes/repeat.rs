use super::*;

#[test]
fn normal_mode_named_register_x_repeats_use_same_register() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
        (Key::X, Modifiers::NONE),
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

    let register_a = vim_named_register(EditorVimNamedRegister {
        index: 0,
        append: false,
    });
    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "af");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("de", EditorVimRegisterKind::Characterwise))
    );
    assert_eq!(
        register_a
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("de", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    vim_clear_named_registers();
    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(5);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
        (Key::X, Modifiers::SHIFT),
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

    let register_a = vim_named_register(EditorVimNamedRegister {
        index: 0,
        append: false,
    });
    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "af");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bc", EditorVimRegisterKind::Characterwise))
    );
    assert_eq!(
        register_a
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bc", EditorVimRegisterKind::Characterwise))
    );
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}
