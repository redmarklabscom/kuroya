use super::*;

#[test]
fn normal_mode_ctrl_n_and_ctrl_p_move_vertically_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..12)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::N,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(down.handled);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());

    for key in [Key::Num4, Key::N] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::N {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 5);
    assert!(pending.is_none());

    let up = handle_vim_editor_key_event(
        &mut buffer,
        Key::P,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(up.handled);
    assert!(!up.changed);
    assert_eq!(buffer.cursor_position().line, 4);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::P] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::P {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            key_event(Key::N, Modifiers::CTRL),
            key_event(Key::Num4, Modifiers::NONE),
            key_event(Key::N, Modifiers::CTRL),
            key_event(Key::P, Modifiers::CTRL),
            key_event(Key::Num3, Modifiers::NONE),
            key_event(Key::P, Modifiers::CTRL),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
