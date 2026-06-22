use super::*;

#[test]
fn normal_mode_ge_moves_to_previous_word_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let previous_end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(go.handled);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(previous_end.handled);
    assert_eq!(previous_end.suppress_text, Some('e'));
    assert_eq!(buffer.cursor(), 15);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::G, Key::E] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_g_shift_e_moves_to_previous_big_word_end_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let go = handle_vim_editor_key_event(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let previous_end = handle_vim_editor_key_event(
        &mut buffer,
        Key::E,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(go.handled);
    assert_eq!(go.suppress_text, Some('g'));
    assert!(previous_end.handled);
    assert_eq!(previous_end.suppress_text, Some('E'));
    assert_eq!(buffer.cursor(), 22);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::G, Key::E] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::E {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 4);
    assert!(pending.is_none());
}
