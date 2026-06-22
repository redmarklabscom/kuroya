use super::*;

#[test]
fn normal_mode_slash_literal_search_accepts_query_and_repeats() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
    let mut buffer = TextBuffer::from_text(1956, None, "alpha beta gamma beta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    for (key, modifiers, suppressed) in [
        (Key::Slash, Modifiers::NONE, Some('/')),
        (Key::B, Modifiers::NONE, Some('b')),
        (Key::E, Modifiers::NONE, Some('e')),
        (Key::T, Modifiers::NONE, Some('t')),
        (Key::A, Modifiers::NONE, Some('a')),
        (Key::Enter, Modifiers::NONE, None),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
        assert_eq!(result.suppress_text, suppressed);
    }

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
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 17));

    let reverse = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    assert!(reverse.handled);
    assert!(!reverse.changed);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 6));
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::Slash, Modifiers::NONE),
            key_event(Key::B, Modifiers::NONE),
            key_event(Key::E, Modifiers::NONE),
            key_event(Key::T, Modifiers::NONE),
            key_event(Key::A, Modifiers::NONE),
            key_event(Key::Enter, Modifiers::NONE),
            key_event(Key::N, Modifiers::NONE),
            key_event(Key::N, Modifiers::SHIFT),
        ],
        EditorVimMode::Normal,
        None,
    ));
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}
