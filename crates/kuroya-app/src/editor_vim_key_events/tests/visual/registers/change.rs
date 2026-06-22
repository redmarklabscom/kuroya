use super::*;

#[test]
fn normal_mode_visual_character_named_register_c_changes_selection() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(4);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let register_a = EditorVimNamedRegister {
        index: 0,
        append: false,
    };

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::H, Modifiers::NONE),
        (Key::H, Modifiers::NONE),
        (Key::Quote, Modifiers::SHIFT),
        (Key::A, Modifiers::NONE),
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
        assert!(!result.changed);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("cde"));
    let change = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(change.handled);
    assert!(change.changed);
    assert_eq!(change.suppress_text, Some('c'));
    assert_eq!(buffer.text(), "abf");
    assert_eq!(buffer.cursor(), 2);
    assert_eq!(mode, EditorVimMode::Insert);
    assert!(pending.is_none());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("cde", EditorVimRegisterKind::Characterwise))
    );
    assert_eq!(
        vim_named_register(register_a)
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("cde", EditorVimRegisterKind::Characterwise))
    );
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::H, Modifiers::NONE),
            key_event(Key::H, Modifiers::NONE),
            key_event(Key::Quote, Modifiers::SHIFT),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::C, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
