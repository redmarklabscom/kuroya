use super::*;

#[test]
fn normal_mode_visual_character_star_and_hash_extend_search_from_active_end() {
    let mut buffer = TextBuffer::from_text(202602, None, "alpha beta alpha gamma alpha".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
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
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha gamma a"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 11,
            cursor: 23,
        })
    );

    pending = None;
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));

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
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha beta a"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 11,
            cursor: 0,
        })
    );
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::Num8, Modifiers::SHIFT),
            Event::Text("*".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
