use super::*;

#[test]
fn normal_mode_vim_settings_remap_custom_key_to_vim_sequence() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(7);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "H".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::H,
        Modifiers::SHIFT,
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
    assert_eq!(result.suppress_text, Some('H'));
    assert_eq!(buffer.cursor(), 0);
    assert_eq!(buffer.text(), "alpha beta");
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_remap_custom_key_to_ctrl_sequence() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha\nbeta\ngamma".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "H".to_owned(),
            after: "<C-n>".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::H,
        Modifiers::SHIFT,
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
    assert_eq!(result.suppress_text, Some('H'));
    assert_eq!(buffer.cursor(), buffer.line_column_to_char(1, 0));
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_remap_home_key_to_vim_sequence() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(0);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Home>".to_owned(),
            after: "$".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    let result = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::Home,
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
    assert_eq!(result.suppress_text, None);
    assert_eq!(buffer.cursor(), 10);
    assert_eq!(mode, EditorVimMode::Normal);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_remap_space_leader_sequence() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(7);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Space>f".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    let leader = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(leader.handled);
    assert!(!leader.changed);
    assert_eq!(leader.suppress_text, Some(' '));
    assert_eq!(
        pending,
        Some(EditorVimPendingKey::CustomKeySequence {
            binding_index: 0,
            matched: 1,
        })
    );
    assert_eq!(
        vim_pending_key_sequence_status_label(pending, &settings).as_deref(),
        Some("leader")
    );
    assert_eq!(buffer.cursor(), 7);

    let complete = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::F,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(complete.handled);
    assert!(!complete.changed);
    assert_eq!(complete.suppress_text, Some('f'));
    assert_eq!(buffer.cursor(), 0);
    assert!(pending.is_none());
}

#[test]
fn normal_mode_vim_settings_space_leader_sequence_scan_detects_remap_mutation() {
    let settings = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Space>d".to_owned(),
            after: "x".to_owned(),
            command: None,
        }],
        ..EditorVimSettings::default()
    };

    assert!(vim_events_include_mutation_with_settings(
        &[
            key_event(Key::Space, Modifiers::NONE),
            key_event(Key::D, Modifiers::NONE)
        ],
        EditorVimMode::Normal,
        None,
        &settings,
    ));
    assert!(!vim_events_include_mutation_with_settings(
        &[
            key_event(Key::Space, Modifiers::NONE),
            key_event(Key::Z, Modifiers::NONE)
        ],
        EditorVimMode::Normal,
        None,
        &settings,
    ));
}

#[test]
fn normal_mode_vim_settings_space_leader_sequence_matches_sibling_bindings() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(3);
    let mut mode = EditorVimMode::Normal;
    let mut pending = None;
    let mut last_char_find = None;
    let mut unnamed_register = None;
    let mut last_change = None;
    let settings = EditorVimSettings {
        key_overrides: vec![
            EditorVimKeyOverride {
                before: "<Space>f".to_owned(),
                after: "0".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "<Space>g".to_owned(),
                after: "$".to_owned(),
                command: None,
            },
        ],
        ..EditorVimSettings::default()
    };

    let leader = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::Space,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );
    let sibling = handle_vim_editor_key_event_with_settings_and_indent(
        &mut buffer,
        Key::G,
        Modifiers::NONE,
        &mut mode,
        &mut pending,
        &mut last_char_find,
        &mut unnamed_register,
        &mut last_change,
        &settings,
        "    ",
    );

    assert!(leader.handled);
    assert!(sibling.handled);
    assert_eq!(sibling.suppress_text, Some('g'));
    assert_eq!(buffer.cursor(), buffer.line_content_end_char(0));
    assert!(pending.is_none());
}
