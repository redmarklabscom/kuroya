use super::*;

#[test]
fn normal_mode_s_substitutes_forward_chars_with_counts() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    let substitute = handle_vim_editor_key_event(
        &mut buffer,
        Key::S,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(substitute.handled);
    assert!(substitute.changed);
    assert_eq!(substitute.suppress_text, Some('s'));
    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "acdef");
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        EditorVimMode::Normal,
        None,
    ));

    buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    mode = EditorVimMode::Normal;
    pending = None;

    for key in [Key::Num3, Key::S] {
        let result =
            handle_vim_editor_key_event(&mut buffer, key, Modifiers::NONE, &mut mode, &mut pending);
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "aef");
    assert!(pending.is_none());
}
