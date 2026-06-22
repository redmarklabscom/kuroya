use super::*;

#[test]
fn normal_mode_star_search_uses_next_word_when_cursor_is_not_on_word() {
    let mut buffer = TextBuffer::from_text(92, None, "alpha, beta beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let from_punctuation = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_punctuation.handled);
    assert!(!from_punctuation.changed);
    assert_eq!(from_punctuation.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 12));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let from_space = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num8,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_space.handled);
    assert!(!from_space.changed);
    assert_eq!(from_space.suppress_text, Some('*'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(0, 12));
    assert!(pending.is_none());
}

#[test]
fn normal_mode_hash_search_uses_next_word_then_searches_backward() {
    let mut buffer = TextBuffer::from_text(93, None, "beta alpha, beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 10));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let from_punctuation = handle_vim_editor_key_event(
        &mut buffer,
        Key::Num3,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(from_punctuation.handled);
    assert!(!from_punctuation.changed);
    assert_eq!(from_punctuation.suppress_text, Some('#'));
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
}
