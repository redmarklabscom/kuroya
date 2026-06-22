use super::*;

#[test]
fn normal_mode_visual_character_x_and_shift_d_delete_selection_aliases() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::V, Key::L] {
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

    assert_eq!(buffer.selected_text().as_deref(), Some("bc"));
    let delete = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(delete.handled);
    assert!(delete.changed);
    assert_eq!(delete.suppress_text, Some('x'));
    assert_eq!(buffer.text(), "adef");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bc", EditorVimRegisterKind::Characterwise))
    );

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::V, Key::L] {
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

    let delete = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::D,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(delete.handled);
    assert!(delete.changed);
    assert_eq!(delete.suppress_text, Some('D'));
    assert_eq!(buffer.text(), "adef");
    assert_eq!(buffer.cursor(), 1);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bc", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::X, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::D, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::X, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
