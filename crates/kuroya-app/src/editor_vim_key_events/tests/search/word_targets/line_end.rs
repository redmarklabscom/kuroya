use super::*;

#[test]
fn normal_mode_star_and_hash_use_previous_word_at_line_end() {
    let mut buffer = TextBuffer::from_text(94, None, "alpha beta\nbeta alpha".to_owned());
    buffer.set_single_cursor(buffer.line_content_end_char(0));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

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
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.line_content_end_char(1));
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
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_star_at_line_end_still_skips_punctuation_only_suffix() {
    let mut buffer = TextBuffer::from_text(95, None, "alpha, beta beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
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
}
