use super::*;

#[test]
fn normal_mode_ctrl_f_and_ctrl_b_move_by_pages_with_counts() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        (0..96)
            .map(|line| format!("line {line}\n"))
            .collect::<Vec<_>>()
            .join(""),
    );
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let page_down = handle_vim_editor_key_event(
        &mut buffer,
        Key::F,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(page_down.handled);
    assert!(!page_down.changed);
    assert_eq!(buffer.cursor_position().line, VIM_DEFAULT_PAGE_SCROLL_LINES);
    assert!(pending.is_none());

    for key in [Key::Num2, Key::F] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::F {
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
        VIM_DEFAULT_PAGE_SCROLL_LINES * 3
    );
    assert!(pending.is_none());

    let page_up = handle_vim_editor_key_event(
        &mut buffer,
        Key::B,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(page_up.handled);
    assert!(!page_up.changed);
    assert_eq!(
        buffer.cursor_position().line,
        VIM_DEFAULT_PAGE_SCROLL_LINES * 2
    );
    assert!(pending.is_none());

    for key in [Key::Num2, Key::B] {
        let result = handle_vim_editor_key_event(
            &mut buffer,
            key,
            if key == Key::B {
                Modifiers::CTRL
            } else {
                Modifiers::NONE
            },
            &mut mode,
            &mut pending,
        );
        assert!(result.handled);
    }
    assert_eq!(buffer.cursor_position().line, 0);
    assert!(pending.is_none());
}
