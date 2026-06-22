use super::*;

#[test]
fn normal_mode_x_deletes_forward_and_counts_as_mutation() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(buffer.text(), "ac");
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::X,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));
}

#[test]
fn normal_mode_shift_x_deletes_backward_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(3);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::X,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert!(result.changed);
    assert_eq!(result.suppress_text, Some('X'));
    assert_eq!(buffer.text(), "abdef");
    assert_eq!(buffer.cursor(), 2);
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::X,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(4);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num3, Key::X] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::X {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "aef");
    assert_eq!(buffer.cursor(), 1);
    assert!(pending.is_none());
}
