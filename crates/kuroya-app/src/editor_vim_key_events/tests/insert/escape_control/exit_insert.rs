use super::*;

#[test]
fn insert_mode_escape_returns_to_normal() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::Escape,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Normal);
}

#[test]
fn insert_mode_ctrl_open_bracket_returns_to_normal() {
    let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
    let mut mode = EditorVimMode::Insert;
    let mut pending = None;

    let result = handle_vim_editor_key_event(
        &mut buffer,
        Key::OpenBracket,
        Modifiers::CTRL,
        &mut mode,
        &mut pending,
    );

    assert!(result.handled);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::OpenBracket,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL,
            },
            Event::Text("x".to_owned()),
        ],
        EditorVimMode::Insert,
        None,
    ));
}
