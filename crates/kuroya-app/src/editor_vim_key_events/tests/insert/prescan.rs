use super::*;

#[test]
fn normal_mode_i_enters_insert_and_suppresses_i_text() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::I,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let mut suppressed = VecDeque::from([result.suppress_text.unwrap()]);

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(
        vim_text_after_suppression("iabc", &mut suppressed).as_deref(),
        Some("abc")
    );
}

#[test]
fn normal_mode_insert_commands_collapse_selection_before_insert() {
    let cases = [
        (Key::I, Modifiers::NONE, "alpha beta", 0, 5, 5),
        (Key::I, Modifiers::SHIFT, "  alpha beta", 2, 5, 2),
        (Key::A, Modifiers::NONE, "alpha beta", 0, 5, 6),
        (Key::A, Modifiers::SHIFT, "alpha beta", 0, 5, 10),
    ];

    for (key, modifiers, text, anchor, cursor, expected_cursor) in cases {
        let mut buffer = TextBuffer::from_text(1, None, text.to_owned());
        buffer.set_selection(anchor, cursor);
        let mut mode = EditorVimMode::Normal;
        let mut pending = None;

        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);

        assert!(result.handled);
        assert_eq!(mode, EditorVimMode::Insert);
        assert!(pending.is_none());
        assert!(!buffer.has_selection());
        assert_eq!(buffer.selections(), &[Selection::caret(expected_cursor)]);
    }
}

#[test]
fn normal_mode_prescan_suppresses_printable_insert_command_text() {
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::I,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("i".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::I,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::I,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("ix".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_prescan_suppresses_printable_pending_key_text() {
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("f".to_owned()),
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_prescan_detects_mutating_printable_pending_key() {
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("r".to_owned()),
            Event::Key {
                key: Key::X,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
