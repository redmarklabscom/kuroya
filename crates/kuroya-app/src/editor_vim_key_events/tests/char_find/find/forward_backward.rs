use super::*;

#[test]
fn normal_mode_f_and_shift_f_find_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "axbxcxd".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let find = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );
    let target = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(find.handled);
    assert!(!find.changed);
    assert_eq!(find.suppress_text, Some('f'));
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some('x'));
    assert_eq!(buffer.cursor(), 1);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::F,
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
            },
        ],
        EditorVimMode::Normal,
        None,
    ));

    buffer.set_single_cursor(0);
    for key in [Key::Num2, Key::F, Key::X] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.len_chars().saturating_sub(1));
    let find_back = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );
    let backward_target = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(find_back.handled);
    assert_eq!(find_back.suppress_text, Some('F'));
    assert!(backward_target.handled);
    assert_eq!(backward_target.suppress_text, Some('x'));
    assert_eq!(buffer.cursor(), 5);
    assert!(pending.is_none());

    buffer.set_single_cursor(buffer.len_chars().saturating_sub(1));
    for (key, modifiers) in [
        (Key::Num2, Modifiers::NONE),
        (Key::F, Modifiers::SHIFT),
        (Key::X, Modifiers::NONE),
    ] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, modifiers, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(buffer.cursor(), 3);
    assert!(pending.is_none());
}
