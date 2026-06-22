use super::*;

#[test]
fn normal_mode_c_word_motions_change_and_enter_insert() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;

    for key in [Key::C, Key::W] {
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
    assert_eq!(buffer.text(), " beta.gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alpha")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::C, Key::Num2, Key::W] {
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
    assert_eq!(buffer.text(), ".gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("alpha beta")
    );
    assert!(pending.is_none());

    buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 10));
    mode = EditorVimMode::Normal;
    pending = None;
    unnamed_register = None;

    for key in [Key::C, Key::B] {
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
    assert_eq!(buffer.text(), "alpha .gamma");
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| register.text.as_str()),
        Some("beta")
    );
    assert!(pending.is_none());
    assert!(vim_events_include_mutation(
        &[
            Event::Key {
                key: Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Num2,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::W,
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
