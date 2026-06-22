use super::*;

#[test]
fn normal_mode_home_and_end_work_as_operator_motions() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::D, Key::Home] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(buffer.text(), "beta");
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("alpha ", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::C, Key::End] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
    }

    assert_eq!(mode, EditorVimMode::Insert);
    assert_eq!(buffer.text(), "alpha ");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::Y, Key::End] {
        let result = handle_vim_editor_key_event_with_state(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
        );
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.text(), "alpha beta");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("beta", EditorVimRegisterKind::Characterwise))
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::D,
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
    assert!(!vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::Y,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Home,
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
