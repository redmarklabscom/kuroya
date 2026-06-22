use super::*;

#[test]
fn normal_mode_uppercase_named_register_appends_substitute_deletes() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::S, Modifiers::NONE),
    ] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }
    let escape = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );
    assert!(escape.handled);

    buffer.set_single_cursor(2);
    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::SHIFT),
        (Key::S, Modifiers::NONE),
    ] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    let register_a = vim_named_register(EditorVimNamedRegister {
        index: 0,
        append: false,
    });
    assert_eq!(buffer.text(), "acef");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("d", EditorVimRegisterKind::Characterwise))
    );
    assert_eq!(
        register_a
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bd", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    vim_clear_named_registers();
    buffer = TextBuffer::from_text(1, None, "alpha\nbeta\ngamma\ndelta\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::S, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }
    let escape = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );
    assert!(escape.handled);

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::SHIFT),
        (Key::S, Modifiers::SHIFT),
    ] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            modifiers,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    let register_a = vim_named_register(EditorVimNamedRegister {
        index: 0,
        append: false,
    });
    assert_eq!(buffer.text(), "\nbeta\n\ndelta\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("gamma\n", EditorVimRegisterKind::Linewise))
    );
    assert_eq!(
        register_a
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha\ngamma\n", EditorVimRegisterKind::Linewise))
    );
    assert!(pending.is_none());
}
