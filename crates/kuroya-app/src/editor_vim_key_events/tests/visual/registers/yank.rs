use super::*;

#[test]
fn normal_mode_visual_character_named_register_y_yanks_selection() {
    vim_clear_named_registers();
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let register_a = EditorVimNamedRegister {
        index: 0,
        append: false,
    };

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

    assert_eq!(buffer.selected_text().as_deref(), Some("bcd"));
    let quote = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Quote,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(quote.handled);
    assert!(!quote.changed);
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
            anchor: 1,
            cursor: 3,
            count: None,
        })
    );

    let register = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::A,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(register.handled);
    assert!(!register.changed);
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor: 1,
            cursor: 3,
            count: None,
            register: register_a,
        })
    );

    let yank = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Y,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(yank.handled);
    assert!(!yank.changed);
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
    assert_eq!(
        vim_named_register(register_a)
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bcd", EditorVimRegisterKind::Characterwise))
    );
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::Quote, Modifiers::SHIFT),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Y, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
