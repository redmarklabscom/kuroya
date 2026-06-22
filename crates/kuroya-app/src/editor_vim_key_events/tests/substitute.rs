use super::*;

#[test]
fn normal_mode_percent_substitute_replaces_all_literal_matches() {
    let mut buffer = TextBuffer::from_text(1, None, "foo foo\nkeep foo".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (index, (key, modifiers, suppressed)) in [
        (Key::Semicolon, Modifiers::SHIFT, Some(':')),
        (Key::Num5, Modifiers::SHIFT, Some('%')),
        (Key::S, Modifiers::NONE, Some('s')),
        (Key::Slash, Modifiers::NONE, Some('/')),
        (Key::F, Modifiers::NONE, Some('f')),
        (Key::O, Modifiers::NONE, Some('o')),
        (Key::O, Modifiers::NONE, Some('o')),
        (Key::Slash, Modifiers::NONE, Some('/')),
        (Key::B, Modifiers::NONE, Some('b')),
        (Key::A, Modifiers::NONE, Some('a')),
        (Key::R, Modifiers::NONE, Some('r')),
        (Key::Slash, Modifiers::NONE, Some('/')),
        (Key::G, Modifiers::NONE, Some('g')),
        (Key::Enter, Modifiers::NONE, None),
    ]
    .into_iter()
    .enumerate()
    {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled, "step {index}");
        assert_eq!(result.suppress_text, suppressed, "step {index}");
        assert_eq!(result.changed, index == 13, "step {index}");
    }

    assert_eq!(buffer.text(), "bar bar\nkeep bar");
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_percent_substitute_shows_pending_command_status() {
    let mut buffer = TextBuffer::from_text(2, None, "foo".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers, status) in [
        (Key::Semicolon, Modifiers::SHIFT, ":"),
        (Key::Num5, Modifiers::SHIFT, ":%"),
        (Key::S, Modifiers::NONE, ":%s"),
        (Key::Slash, Modifiers::NONE, ":%s/"),
        (Key::F, Modifiers::NONE, ":%s/f"),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
        assert_eq!(
            vim_pending_command_status_label(pending).as_deref(),
            Some(status)
        );
    }

    let cancel = handle_vim_editor_key_event(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(cancel.handled);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_percent_substitute_counts_as_possible_mutation() {
    assert!(vim_events_include_mutation(
        &[
            key_event(Key::Semicolon, Modifiers::SHIFT),
            key_event(Key::Num5, Modifiers::SHIFT),
            key_event(Key::S, Modifiers::NONE),
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::F, Modifiers::NONE),
            key_event(Key::O, Modifiers::NONE),
            key_event(Key::O, Modifiers::NONE),
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::R, Modifiers::NONE),
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
