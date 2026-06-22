use super::*;

#[test]
fn normal_mode_visual_character_n_and_shift_n_extend_last_search() {
    let mut buffer = TextBuffer::from_text(202601, None, "alpha beta alpha gamma alpha".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

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
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 11));
    assert!(pending.is_none());

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

    let next = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(next.handled);
    assert!(!next.changed);
    assert_eq!(next.suppress_text, Some('n'));
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

    let previous = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(previous.handled);
    assert!(!previous.changed);
    assert_eq!(previous.suppress_text, Some('N'));
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
            key_event(Key::N, Modifiers::NONE)
        ],
        EditorVimMode::Normal,
        None,
    ));
}
