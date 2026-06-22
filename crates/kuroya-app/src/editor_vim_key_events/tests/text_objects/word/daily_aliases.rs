use super::*;

#[test]
fn normal_mode_daily_word_text_objects_support_ciw_diw_yiw() {
    let mut last_char_find = None;

    let mut buffer = TextBuffer::from_text(10, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut unnamed_register = None;

    for key in [Key::C, Key::I, Key::W] {
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
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha  gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(11, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::D, Key::I, Key::W] {
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
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "alpha  gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(12, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 7));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::Y, Key::I, Key::W] {
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
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "alpha beta gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta")
    );
    assert!(pending.is_none());
}
