use super::*;

#[test]
fn normal_mode_visual_character_char_find_extends_with_counts_and_repeats() {
    let mut buffer = TextBuffer::from_text(202606, None, "abxcxdxe".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::V, Key::Num2, Key::F] {
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

    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterCharFind {
            anchor: 0,
            cursor: 0,
            count: Some(2),
            motion: EditorVimCharFindMotion::FindForward,
        })
    );

    let target = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some('x'));
    assert_eq!(buffer.selected_text().as_deref(), Some("abxcx"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 4,
        })
    );
    assert_eq!(
        last_char_find,
        Some(EditorVimCharFind {
            motion: EditorVimCharFindMotion::FindForward,
            target: 'x',
        })
    );

    let repeat = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Semicolon,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(repeat.suppress_text, Some(';'));
    assert_eq!(buffer.selected_text().as_deref(), Some("abxcxdx"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 6,
        })
    );

    let reverse = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Comma,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(reverse.suppress_text, Some(','));
    assert_eq!(buffer.selected_text().as_deref(), Some("abxcx"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 4,
        })
    );
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::F, Modifiers::NONE),
            Event::Text("f".to_owned()),
            key_event(Key::X, Modifiers::NONE),
            Event::Text("x".to_owned()),
            key_event(Key::Semicolon, Modifiers::NONE),
            Event::Text(";".to_owned()),
            key_event(Key::Comma, Modifiers::NONE),
            Event::Text(",".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_visual_character_char_find_backward_and_till_use_command_targets() {
    let mut buffer = TextBuffer::from_text(202607, None, "abxcxdxe".to_owned());
    buffer.set_single_cursor(7);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for (key, modifiers) in [
        (Key::V, Modifiers::NONE),
        (Key::F, Modifiers::SHIFT),
        (Key::X, Modifiers::NONE),
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

    assert_eq!(buffer.selected_text().as_deref(), Some("xe"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 7,
            cursor: 6,
        })
    );
    assert_eq!(
        last_char_find,
        Some(EditorVimCharFind {
            motion: EditorVimCharFindMotion::FindBackward,
            target: 'x',
        })
    );

    let till = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::T,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(till.handled);
    assert!(!till.changed);
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterCharFind {
            anchor: 7,
            cursor: 6,
            count: None,
            motion: EditorVimCharFindMotion::TillBackward,
        })
    );

    let target = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some('c'));
    assert_eq!(buffer.text(), "abxcxdxe");
    assert_eq!(buffer.selected_text().as_deref(), Some("xdxe"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 7,
            cursor: 4,
        })
    );
    assert_eq!(
        last_char_find,
        Some(EditorVimCharFind {
            motion: EditorVimCharFindMotion::TillBackward,
            target: 'c',
        })
    );
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::F, Modifiers::SHIFT),
            Event::Text("F".to_owned()),
            key_event(Key::X, Modifiers::NONE),
            Event::Text("x".to_owned()),
            key_event(Key::T, Modifiers::SHIFT),
            Event::Text("T".to_owned()),
            key_event(Key::C, Modifiers::NONE),
            Event::Text("c".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
