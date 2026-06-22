use super::*;

#[test]
fn normal_mode_f_accepts_shifted_punctuation_targets() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha:beta".to_owned());
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
        Key::Semicolon,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
    );

    assert!(find.handled);
    assert_eq!(find.suppress_text, Some('f'));
    assert!(target.handled);
    assert!(!target.changed);
    assert_eq!(target.suppress_text, Some(':'));
    assert_eq!(buffer.cursor(), 5);
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
                key: Key::Semicolon,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
        ],
        EditorVimMode::Normal,
        None,
    ));
}
