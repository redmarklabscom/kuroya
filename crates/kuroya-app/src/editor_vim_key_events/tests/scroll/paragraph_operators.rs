use super::*;

#[test]
fn normal_mode_brace_operator_motions_delete_and_change_paragraphs() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha\n\nbeta\n\ncharlie\n\ndelta".to_owned());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;

    for key in [Key::D, Key::Num2, Key::CloseBracket] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            if key == Key::CloseBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Normal);
    assert_eq!(buffer.text(), "charlie\n\ndelta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha\n\nbeta\n\n", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "one\n\nalpha\nbeta\n\nomega".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(5, 0));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;
    last_change = None;

    for key in [Key::C, Key::OpenBracket] {
        let result = handle_vim_editor_key_event_with_repeat_state(
            &mut buffer,
            key,
            if key == Key::OpenBracket {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "one\n\nomega");
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(2, 0));
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha\nbeta\n\n", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
}
