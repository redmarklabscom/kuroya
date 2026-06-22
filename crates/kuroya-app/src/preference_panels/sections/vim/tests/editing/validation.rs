use super::super::super::{
    VimBindingOwner,
    editing::{
        vim_binding_edit_error, vim_disabled_binding_edit_error, vim_override_before_edit_error,
    },
};
use kuroya_core::{Command, EditorVimKeyOverride, EditorVimSettings};

#[test]
fn vim_binding_editor_rejects_keys_tied_to_other_bindings() {
    let vim = EditorVimSettings::default();

    assert_eq!(
        vim_binding_edit_error("j", VimBindingOwner::BuiltIn("h"), &vim),
        Some("Already used by Move down".to_owned())
    );
    assert_eq!(
        vim_binding_edit_error("2", VimBindingOwner::BuiltIn("h"), &vim),
        Some("Already used by Count prefix 2".to_owned())
    );
    assert_eq!(
        vim_binding_edit_error("<C-r>", VimBindingOwner::BuiltIn("h"), &vim),
        Some("Already used by Redo".to_owned())
    );
    assert_eq!(
        vim_binding_edit_error("gg", VimBindingOwner::BuiltIn("h"), &vim),
        Some("Use one supported Vim key".to_owned())
    );
}

#[test]
fn vim_binding_editor_rejects_keys_tied_to_disabled_bindings() {
    let vim = EditorVimSettings {
        disabled_bindings: vec!["H".to_owned()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    };

    assert_eq!(
        vim_binding_edit_error("H", VimBindingOwner::BuiltIn("h"), &vim),
        Some("Already used by disabled binding 1".to_owned())
    );
    assert_eq!(
        vim_binding_edit_error("H", VimBindingOwner::CustomOverride(0), &vim),
        Some("Already used by disabled binding 1".to_owned())
    );
}

#[test]
fn custom_disabled_binding_editor_rejects_keys_tied_to_other_bindings() {
    let vim = EditorVimSettings {
        disabled_bindings: vec![String::new()],
        key_overrides: vec![EditorVimKeyOverride {
            before: "K".to_owned(),
            after: String::new(),
            command: Some(Command::RequestHover),
        }],
    };

    assert_eq!(
        vim_disabled_binding_edit_error("j", VimBindingOwner::CustomDisabled(0), &vim),
        Some("Already used by Move down".to_owned())
    );
    assert_eq!(
        vim_disabled_binding_edit_error("K", VimBindingOwner::CustomDisabled(0), &vim),
        Some("Already used by custom command 1 (Show Hover)".to_owned())
    );
    assert_eq!(
        vim_disabled_binding_edit_error("Q", VimBindingOwner::CustomDisabled(0), &vim),
        None
    );
}

#[test]
fn custom_override_before_editor_accepts_supported_key_sequences() {
    let vim = EditorVimSettings::default();

    assert_eq!(
        vim_override_before_edit_error("<Space>f", VimBindingOwner::CustomOverride(0), &vim),
        None
    );
    assert_eq!(
        vim_disabled_binding_edit_error("<Space>f", VimBindingOwner::CustomDisabled(0), &vim),
        Some("Use one supported Vim key".to_owned())
    );
    assert_eq!(
        vim_override_before_edit_error("<Nope>", VimBindingOwner::CustomOverride(0), &vim),
        Some("Use supported Vim keys".to_owned())
    );
}

#[test]
fn vim_binding_conflict_labels_use_visible_custom_row_numbers() {
    let vim = EditorVimSettings {
        disabled_bindings: vec!["h".to_owned(), "H".to_owned()],
        key_overrides: vec![
            EditorVimKeyOverride {
                before: "L".to_owned(),
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

    assert_eq!(
        vim_binding_edit_error("H", VimBindingOwner::BuiltIn("j"), &vim),
        Some("Already used by disabled binding 1".to_owned())
    );
    assert_eq!(
        vim_disabled_binding_edit_error("K", VimBindingOwner::CustomDisabled(1), &vim),
        Some("Already used by custom command 1 (Show Hover)".to_owned())
    );
}
