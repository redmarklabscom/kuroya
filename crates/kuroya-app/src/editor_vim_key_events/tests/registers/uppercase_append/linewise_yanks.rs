use super::*;

#[test]
fn normal_mode_uppercase_named_register_appends_linewise_yanks() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "alpha\nbeta\ngamma".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::Y, Modifiers::NONE),
        (Key::Y, Modifiers::NONE),
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
        assert!(!result.changed);
    }

    buffer.set_single_cursor(buffer.line_column_to_char(1, 0));
    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::SHIFT),
        (Key::Y, Modifiers::NONE),
        (Key::Y, Modifiers::NONE),
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
        assert!(!result.changed);
    }

    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta\n", EditorVimRegisterKind::Linewise))
    );

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::P, Modifiers::NONE),
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

    assert_eq!(buffer.text(), "alpha\nbeta\ngamma\nalpha\nbeta\n");
    assert!(pending.is_none());
}
