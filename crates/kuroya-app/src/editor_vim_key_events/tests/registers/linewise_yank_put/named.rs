use super::*;

#[test]
fn normal_mode_named_register_yank_lines_and_put_separately_from_unnamed() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "alpha\nbeta\ngamma\ndelta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 0));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

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

    assert_eq!(buffer.text(), "alpha\nbeta\ngamma\ndelta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta\n", EditorVimRegisterKind::Linewise))
    );

    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    for key in [Key::D, Key::D] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "alpha\nbeta\ndelta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("gamma\n", EditorVimRegisterKind::Linewise))
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

    assert_eq!(buffer.text(), "alpha\nbeta\ndelta\nbeta\n");
    assert_eq!(buffer.cursor_position().line, 3);
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Quote,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::P,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
