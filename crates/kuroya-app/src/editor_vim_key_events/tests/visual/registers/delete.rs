use super::*;

#[test]
fn normal_mode_visual_character_named_register_d_deletes_selection() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(4);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let register_a = EditorVimNamedRegister {
        index: 0,
        append: false,
    };

    for key in [Key::V, Key::H, Key::H] {
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

    assert_eq!(buffer.selected_text().as_deref(), Some("cde"));
    for (key, modifiers) in [(Key::Quote, Modifiers::SHIFT), (Key::A, Modifiers::NONE)] {
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

    let delete = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::D,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(delete.handled);
    assert!(delete.changed);
    assert_eq!(buffer.text(), "abf");
    assert_eq!(buffer.cursor(), 2);
    assert_eq!(mode, EditorVimMode::Normal);
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
            key_event(Key::D, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
