use super::*;

#[test]
fn normal_mode_space_can_be_char_find_target() {
    let mut buffer = TextBuffer::from_text(1, None, "ab cd ef".to_owned());
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
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(find.handled);
    assert!(!find.changed);
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some(' '));
    assert_eq!(buffer.cursor(), 2);
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
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(" ".to_owned()),
        ],
        EditorVimMode::Normal,
        None,
    ));
}
