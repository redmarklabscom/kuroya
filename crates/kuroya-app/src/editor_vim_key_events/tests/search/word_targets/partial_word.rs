use super::*;

#[test]
fn normal_mode_g_star_searches_partial_word_matches_with_counts_and_repeat() {
    let mut buffer = TextBuffer::from_text(1901, None, "alpha alphabet alpha alphabet".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let count = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num2,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(count.handled);
    assert!(!count.changed);

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(go.handled);
    assert!(!go.changed);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(pending.is_some());

    let star = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(star.handled);
    assert!(!star.changed);
    assert_eq!(star.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert!(pending.is_none());

    let repeat = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(repeat.suppress_text, Some('n'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 21));

    let reverse = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(reverse.suppress_text, Some('N'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 15));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Num2,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::G,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Num8,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_g_hash_searches_partial_word_matches_and_repeats_backward() {
    let mut buffer = TextBuffer::from_text(1902, None, "alpha alphabet alpha alphabet".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 15));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(go.handled);
    assert!(!go.changed);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(pending.is_some());

    let hash = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(hash.handled);
    assert!(!hash.changed);
    assert_eq!(hash.suppress_text, Some('#'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(pending.is_none());

    let repeat = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    assert!(repeat.handled);
    assert!(!repeat.changed);
    assert_eq!(repeat.suppress_text, Some('n'));
    assert_eq!(buffer.cursor(), 0);

    let reverse = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(reverse.suppress_text, Some('N'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(pending.is_none());

    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::G,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Num3,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
