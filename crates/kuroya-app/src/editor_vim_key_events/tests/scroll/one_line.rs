use super::*;

#[test]
fn normal_mode_ctrl_e_and_ctrl_y_move_one_line_with_counts() {
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
        Key::E,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(down.handled);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::E] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::E {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 4);
    assert!(pending.is_none());

    let up = handle_vim_editor_key_event(
        &mut buffer,
        Key::Y,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(up.handled);
    assert!(!up.changed);
    assert_eq!(buffer.cursor_position().line, 3);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::Y] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::Y {
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
            key_event(Key::E, Modifiers::CTRL),
            key_event(Key::Num3, Modifiers::NONE),
            key_event(Key::E, Modifiers::CTRL),
            key_event(Key::Y, Modifiers::CTRL),
            key_event(Key::Num2, Modifiers::NONE),
            key_event(Key::Y, Modifiers::CTRL),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
