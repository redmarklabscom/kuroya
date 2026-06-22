use super::*;

#[test]
fn normal_mode_visual_character_starts_from_visible_char_at_line_and_buffer_end() {
    let mut buffer = TextBuffer::from_text(1, None, "abc\nnext".to_owned());
    buffer.set_single_cursor(buffer.line_content_end_char(0));
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
    assert_eq!(buffer.selected_text().as_deref(), Some("c"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 2,
            cursor: 2,
        })
    );

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
    assert_eq!(buffer.text(), "ab\nnext");
    assert_eq!(buffer.cursor(), 2);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("c", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

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
    assert_eq!(buffer.selected_text().as_deref(), Some("c"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 2,
            cursor: 2,
        })
    );

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
    assert_eq!(buffer.text(), "ab");
    assert_eq!(buffer.cursor(), 2);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("c", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::D, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
