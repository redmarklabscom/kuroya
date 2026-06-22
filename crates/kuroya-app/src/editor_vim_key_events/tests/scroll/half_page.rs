use super::*;

#[test]
fn normal_mode_ctrl_d_and_ctrl_u_move_vertically_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..24)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let down = handle_vim_editor_key_event(
        &mut buffer,
        Key::D,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(down.handled);
    assert!(!down.changed);
    assert_eq!(buffer.cursor_position().line, VIM_DEFAULT_CTRL_SCROLL_LINES);
    assert!(pending.is_none());

    for key in [Key::Num3, Key::D] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::D {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(
        buffer.cursor_position().line,
        VIM_DEFAULT_CTRL_SCROLL_LINES + 3
    );
    assert!(pending.is_none());

    let up = handle_vim_editor_key_event(
        &mut buffer,
        Key::U,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(up.handled);
    assert!(!up.changed);
    assert_eq!(buffer.cursor_position().line, 3);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::U] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::U {
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
}
