use super::super::super::editing::{
    apply_custom_override_after_edit, binding_is_builtin_override, command_vim_key_override,
    custom_override_indices, default_custom_disabled_vim_binding, switch_vim_override_to_keys,
    vim_key_override_to_keys,
};
use kuroya_core::{Command, EditorVimKeyOverride, EditorVimSettings};

#[test]
fn custom_override_after_manual_edit_stores_normalized_valid_sequences() {
    let mut vim = EditorVimSettings {
        disabled_bindings: Vec::new(),
        key_overrides: vec![EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    };

    assert_eq!(apply_custom_override_after_edit(&mut vim, 0, " gg "), None);
    assert_eq!(vim.key_overrides[0].after, "gg");

    assert_eq!(
        apply_custom_override_after_edit(&mut vim, 0, "<Nope>"),
        Some("Use supported Vim keys".to_owned())
    );
    assert_eq!(vim.key_overrides[0].after, "gg");

    assert_eq!(
        apply_custom_override_after_edit(&mut vim, 0, " "),
        Some("After must contain Vim keys".to_owned())
    );
    assert_eq!(vim.key_overrides[0].after, "gg");
}

#[test]
fn custom_override_after_manual_edit_rejects_parseable_unhandled_sequences() {
    let mut vim = EditorVimSettings {
        disabled_bindings: Vec::new(),
        key_overrides: vec![EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    };

    assert_eq!(
        apply_custom_override_after_edit(&mut vim, 0, "<Left>"),
        Some("Use supported Vim keys".to_owned())
    );
    assert_eq!(vim.key_overrides[0].after, "0");
}

#[test]
fn custom_vim_remaps_start_with_default_after_sequence() {
    let vim = EditorVimSettings::default();
    let binding = vim_key_override_to_keys(&vim).expect("default remap should be available");

    assert_eq!(binding.before, "q");
    assert_eq!(binding.after, "0");
    assert!(binding.command.is_none());

    let vim = EditorVimSettings {
        key_overrides: vec![binding],
        ..EditorVimSettings::default()
    };
    assert_eq!(custom_override_indices(&vim), [0]);
}

#[test]
fn custom_vim_rows_pick_next_free_default_before_key() {
    let vim = EditorVimSettings {
        disabled_bindings: vec!["q".to_owned()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "z".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    };

    assert_eq!(
        default_custom_disabled_vim_binding(&vim),
        Some("Q".to_owned())
    );
    assert_eq!(
        vim_key_override_to_keys(&vim)
            .expect("free override key")
            .before,
        "Q"
    );
}

#[test]
fn custom_vim_rows_do_not_create_empty_defaults_when_candidates_are_exhausted() {
    let vim = EditorVimSettings {
        disabled_bindings: ["q", "z", "Q", "Z", "<Left>", "<Right>", "<Up>", "<Down>"]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        key_overrides: Vec::new(),
    };

    assert_eq!(default_custom_disabled_vim_binding(&vim), None);
    assert_eq!(vim_key_override_to_keys(&vim), None);
    assert_eq!(command_vim_key_override(&vim), None);
}

#[test]
fn custom_override_target_switch_to_keys_preserves_valid_row() {
    let mut binding = EditorVimKeyOverride {
        before: "K".to_owned(),
        after: String::new(),
        command: Some(Command::RequestHover),
    };

    switch_vim_override_to_keys(&mut binding);

    assert_eq!(
        binding,
        EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }
    );
}

#[test]
fn custom_override_target_switch_to_keys_resets_invalid_after_sequence() {
    let mut binding = EditorVimKeyOverride {
        before: "K".to_owned(),
        after: "<Nope>".to_owned(),
        command: Some(Command::RequestHover),
    };

    switch_vim_override_to_keys(&mut binding);

    assert_eq!(
        binding,
        EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }
    );
}

#[test]
fn custom_vim_override_indices_hide_builtin_remaps_but_keep_commands() {
    let vim = EditorVimSettings {
        disabled_bindings: vec!["h".to_owned()],
        key_overrides: vec![
            EditorVimKeyOverride {
                before: "H".to_owned(),
                after: "h".to_owned(),
                command: None,
            },
            EditorVimKeyOverride {
                before: "K".to_owned(),
                after: String::new(),
                command: Some(Command::RequestHover),
            },
        ],
    };

    assert!(binding_is_builtin_override(0, &vim.key_overrides[0], &vim));
    assert_eq!(custom_override_indices(&vim), [1]);
}
