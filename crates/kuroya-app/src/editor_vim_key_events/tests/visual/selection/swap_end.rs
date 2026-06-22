use super::*;

#[test]
fn normal_mode_visual_character_o_swaps_active_selection_end() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
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

    assert_eq!(buffer.selected_text().as_deref(), Some("bcd"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 1,
            cursor: 3,
        })
    );

    let swap = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::O,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(swap.handled);
    assert!(!swap.changed);
    assert_eq!(swap.suppress_text, Some('o'));
    assert_eq!(buffer.selected_text().as_deref(), Some("bcd"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 3,
            cursor: 1,
        })
    );

    let left = handle_vim_editor_key_event_with_state(
        &mut buffer,
        Key::H,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
    );

    assert!(left.handled);
    assert!(!left.changed);
    assert_eq!(buffer.selected_text().as_deref(), Some("abcd"));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter {
            anchor: 3,
            cursor: 0,
        })
    );
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::V, Modifiers::NONE),
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::L, Modifiers::NONE),
            key_event(Key::O, Modifiers::NONE),
            Event::Text("o".to_owned()),
            key_event(Key::H, Modifiers::NONE),
            Event::Text("h".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
