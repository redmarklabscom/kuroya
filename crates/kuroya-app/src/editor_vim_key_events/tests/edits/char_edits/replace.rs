use super::*;

#[test]
fn normal_mode_r_replaces_forward_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let replace = handle_vim_editor_key_event(
        &mut buffer,
        Key::R,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let replacement = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(replace.handled);
    assert!(!replace.changed);
    assert_eq!(replace.suppress_text, Some('r'));
    assert_eq!(pending, None);
    assert!(replacement.handled);
    assert!(replacement.changed);
    assert_eq!(replacement.suppress_text, Some('x'));
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "axcdef");
    assert_eq!(buffer.cursor(), 1);
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num3, Key::R, Key::X] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "axxxef");
    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_r_accepts_shifted_punctuation_replacements() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let replace = handle_vim_editor_key_event(
        &mut buffer,
        Key::R,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let replacement = handle_vim_editor_key_event(
        &mut buffer,
        Key::Quote,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(replace.handled);
    assert_eq!(replace.suppress_text, Some('r'));
    assert!(replacement.handled);
    assert!(replacement.changed);
    assert_eq!(replacement.suppress_text, Some('"'));
    assert_eq!(buffer.text(), "a\"c");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Quote,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            }
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_r_accepts_enter_replacement_and_repeats() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    let replace = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::R,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );
    let replacement = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Enter,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(replace.handled);
    assert!(!replace.changed);
    assert!(replacement.handled);
    assert!(replacement.changed);
    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "a\ncdef");
    assert!(pending.is_none());
    assert!(last_change.is_some());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer.set_single_cursor(buffer.line_column_to_char(1, 0));
    let repeat = handle_vim_editor_key_event_with_repeat_state(
        &mut buffer,
        Key::Period,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
    );

    assert!(repeat.handled);
    assert!(repeat.changed);
    assert_eq!(buffer.text(), "a\n\ndef");
    assert!(pending.is_none());
}
