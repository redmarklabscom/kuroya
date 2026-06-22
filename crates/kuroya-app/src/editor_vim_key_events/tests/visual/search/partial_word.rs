use super::*;

#[test]
fn normal_mode_visual_character_g_star_and_g_hash_extend_partial_word_search() {
    let mut buffer =
        TextBuffer::from_text(202603, None, "alpha alphabet alpha alphabet".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    let visual = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::V,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );
    assert!(visual.handled);
    assert!(!visual.changed);

    let go = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );
    assert!(go.handled);
    assert!(!go.changed);
    assert_eq!(go.suppress_text, Some('g'));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterGo {
            anchor: 0,
            cursor: 0,
            count: None,
        })
    );

    let star = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(star.handled);
    assert!(!star.changed);
    assert_eq!(star.suppress_text, Some('*'));
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha a"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 6,
        })
    );

    let repeat = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(repeat.suppress_text, Some('n'));
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha alphabet a"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 15,
        })
    );

    pending = None;
    buffer.set_single_cursor(buffer.line_column_to_char(0, 15));

    let visual = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::V,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );
    assert!(visual.handled);
    assert!(!visual.changed);

    let go = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );
    assert!(go.handled);
    assert!(!go.changed);

    let hash = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(hash.handled);
    assert!(!hash.changed);
    assert_eq!(hash.suppress_text, Some('#'));
    assert_eq!(buffer.selected_text().as_deref(), Some("alphabet a"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 15,
            cursor: 6,
        })
    );
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            Event::Text("g".to_owned()),
            key_event(Key::Num8, Modifiers::SHIFT),
            Event::Text("*".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
