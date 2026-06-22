use super::*;

#[test]
fn normal_mode_cc_changes_current_line_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let first = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let second = handle_vim_editor_key_event(
        &mut buffer,
        Key::C,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(first.handled);
    assert!(!first.changed);
    assert_eq!(first.suppress_text, Some('c'));
    assert!(second.changed);
    assert_eq!(second.suppress_text, Some('c'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nthree\nfour\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        Some(super::EditorVimPendingKey::ChangeLine(1)),
    ));

    buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::C, Key::C] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nfour\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_shift_s_changes_current_line_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let change = handle_vim_editor_key_event(
        &mut buffer,
        Key::S,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(change.handled);
    assert!(change.changed);
    assert_eq!(change.suppress_text, Some('S'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nthree\nfour\n");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num2, Key::S] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::S {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nfour\n");
    assert!(pending.is_none());
}

#[test]
fn normal_mode_change_line_preserves_last_and_single_blank_line() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for key in [Key::C, Key::C] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n");
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "only".to_owned());
    buffer.set_single_cursor(0);
    mode = EditorVimMode::Normal;
    pending = None;

    let change = handle_vim_editor_key_event(
        &mut buffer,
        Key::S,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(change.handled);
    assert!(change.changed);
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "");
    assert!(pending.is_none());
}
