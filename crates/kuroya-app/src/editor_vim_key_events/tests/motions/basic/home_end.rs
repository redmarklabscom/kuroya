use super::*;

#[test]
fn normal_mode_home_and_end_mirror_line_start_and_end_motions() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo words\nthree\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 4));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let home = handle_vim_editor_key_event(
        &mut buffer,
        Key::Home,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(home.handled);
    assert!(!home.changed);
    assert_eq!(home.suppress_text, None);
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert!(pending.is_none());

    for key in [Key::Num2, Key::End] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.cursor(), buffer.line_content_end_char(2));
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Home,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::End,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
