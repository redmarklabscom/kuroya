use super::*;

#[test]
fn normal_mode_visual_character_shift_y_yanks_selection() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(202604, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::V, Key::Num2, Key::L] {
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
        assert!(!result.changed);
    }

    let yank = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Y,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(yank.handled);
    assert!(!yank.changed);
    assert_eq!(yank.suppress_text, Some('Y'));
    assert_eq!(buffer.text(), "abcdef");
    assert_eq!(buffer.cursor(), 1);
    assert!(!buffer.has_selection());
    assert!(pending.is_none());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bcd", EditorVimRegisterKind::Characterwise))
    );

    let register_b = EditorVimNamedRegister {
        index: 1,
        append: false,
    };
    buffer = TextBuffer::from_text(202605, None, "abcdef".to_owned());
    buffer.set_single_cursor(2);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::Quote, Modifiers::SHIFT),
        (Key::B, Modifiers::NONE),
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

    let yank = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Y,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(yank.handled);
    assert!(!yank.changed);
    assert_eq!(yank.suppress_text, Some('Y'));
    assert_eq!(buffer.text(), "abcdef");
    assert_eq!(buffer.cursor(), 2);
    assert!(!buffer.has_selection());
    assert!(pending.is_none());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("cd", EditorVimRegisterKind::Characterwise))
    );
    assert_eq!(
        vim_named_register(register_b)
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("cd", EditorVimRegisterKind::Characterwise))
    );
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::Y, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
