use super::*;

#[test]
fn visual_mode_vim_settings_disable_visual_delete_binding() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        disabled_bindings: vec!["x".to_owned()],
        ..EditorVimSettings::default()
    };

    for key in [Key::V, Key::L] {
        let result = handle_vim_editor_key_event_with_settings_and_indent(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
            &settings,
            "    ",
        );
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("bc"));
    let disabled_delete = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::X,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(disabled_delete.handled);
    assert!(!disabled_delete.changed);
    assert_eq!(disabled_delete.suppress_text, Some('x'));
    assert_eq!(buffer.text(), "abcdef");
    assert_eq!(buffer.selected_text().as_deref(), Some("bc"));
    assert!(matches!(
        pending,
        Some(EditorVimPendingKey::VisualCharacter { .. })
    ));
}

#[test]
fn visual_mode_vim_settings_remap_custom_key_to_visual_delete() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    buffer.set_single_cursor(1);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "Q".to_owned(),
            after: "x".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    for key in [Key::V, Key::L] {
        let result = handle_vim_editor_key_event_with_settings_and_indent(
            &mut buffer,
            key,
            Modifiers::NONE,
            &mut mode,
            &mut pending,
            &mut last_char_find,
            &mut unnamed_register,
            &mut last_change,
            &settings,
            "    ",
        );
        assert!(result.handled);
        assert!(!result.changed);
    }

    assert_eq!(buffer.selected_text().as_deref(), Some("bc"));
    let remapped_delete = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::Q,
        Modifiers::SHIFT,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(remapped_delete.handled);
    assert!(remapped_delete.changed);
    assert_eq!(remapped_delete.suppress_text, Some('Q'));
    assert_eq!(buffer.text(), "adef");
    assert_eq!(buffer.cursor(), 1);
    assert!(pending.is_none());
    assert_eq!(
        unnamed_register
            .as_ref()
            .map(|register| (register.text.as_str(), register.kind)),
        Some(("bc", EditorVimRegisterKind::Characterwise))
    );
}
