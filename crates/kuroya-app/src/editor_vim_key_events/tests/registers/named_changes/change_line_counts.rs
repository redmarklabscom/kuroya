use super::*;

#[test]
fn normal_mode_named_register_cc_changes_lines_with_counts_and_repeat() {
    vim_clear_named_registers();
    let mut buffer =
        TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\nfive\nsix\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for (key, modifiers) in [
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
        (Key::C, Modifiers::NONE),
        (Key::C, Modifiers::NONE),
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

    let register_a = vim_named_register(EditorVimNamedRegister {
        index: 0,
        append: false,
    });
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nfour\nfive\nsix\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("two\nthree\n", EditorVimRegisterKind::Linewise))
    );
    assert_eq!(
        register_a
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("two\nthree\n", EditorVimRegisterKind::Linewise))
    );
    assert!(pending.is_none());

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
    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
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
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\n\nsix\n");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("four\nfive\n", EditorVimRegisterKind::Linewise))
    );
    assert_eq!(
        register_a
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("four\nfive\n", EditorVimRegisterKind::Linewise))
    );
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
                key: Key::Num2,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::C,
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
