use super::super::super::{
    VimBindingOwner,
    editing::{
        binding_is_builtin_override, custom_override_indices, disable_builtin_vim_binding,
        reset_builtin_vim_binding, set_builtin_vim_binding, vim_binding_edit_error,
        vim_builtin_effective_binding,
    },
};
use kuroya_core::{EditorVimKeyOverride, EditorVimSettings};

#[test]
fn builtin_vim_binding_editor_remaps_disables_and_resets_binding() {
    let mut vim = EditorVimSettings::default();

    set_builtin_vim_binding(&mut vim, "h", "H".to_owned());

    assert_eq!(vim_builtin_effective_binding(&vim, "h"), "H");
    assert!(vim.disabled_bindings.iter().any(|binding| binding == "h"));
    assert_eq!(
        vim.key_overrides,
        [EditorVimKeyOverride {
            before: "H".to_owned(),
            after: "h".to_owned(),
            command: None,
        }]
    );

    disable_builtin_vim_binding(&mut vim, "h");
    assert_eq!(vim_builtin_effective_binding(&vim, "h"), "");
    assert!(vim.key_overrides.is_empty());

    reset_builtin_vim_binding(&mut vim, "h");
    assert_eq!(vim_builtin_effective_binding(&vim, "h"), "h");
    assert!(vim.disabled_bindings.is_empty());
}

#[test]
fn custom_vim_remaps_to_builtin_targets_stay_visible_and_survive_builtin_reset() {
    let mut vim = EditorVimSettings {
        disabled_bindings: Vec::new(),
        key_overrides: vec![EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }],
    };

    assert!(!binding_is_builtin_override(0, &vim.key_overrides[0], &vim));
    assert_eq!(custom_override_indices(&vim), [0]);

    reset_builtin_vim_binding(&mut vim, "0");

    assert_eq!(custom_override_indices(&vim), [0]);
    assert_eq!(vim.key_overrides[0].before, "K");
    assert_eq!(vim.key_overrides[0].after, "0");

    set_builtin_vim_binding(&mut vim, "0", "H".to_owned());

    assert_eq!(vim_builtin_effective_binding(&vim, "0"), "H");
    assert_eq!(custom_override_indices(&vim), [1]);
    assert_eq!(vim.key_overrides[1].before, "K");
    assert_eq!(vim.key_overrides[1].after, "0");

    reset_builtin_vim_binding(&mut vim, "0");

    assert_eq!(vim_builtin_effective_binding(&vim, "0"), "0");
    assert_eq!(custom_override_indices(&vim), [0]);
    assert_eq!(
        vim.key_overrides,
        [EditorVimKeyOverride {
            before: "K".to_owned(),
            after: "0".to_owned(),
            command: None,
        }]
    );
}

#[test]
fn builtin_vim_binding_editor_allows_text_restoring_disabled_default() {
    let mut vim = EditorVimSettings::default();

    disable_builtin_vim_binding(&mut vim, "h");

    assert_eq!(vim_builtin_effective_binding(&vim, "h"), "");
    assert_eq!(
        vim_binding_edit_error("h", VimBindingOwner::BuiltIn("h"), &vim),
        None
    );
    assert_eq!(
        vim_binding_edit_error("h", VimBindingOwner::BuiltIn("j"), &vim),
        Some("Already used by disabled binding 1".to_owned())
    );

    set_builtin_vim_binding(&mut vim, "h", "h".to_owned());

    assert_eq!(vim_builtin_effective_binding(&vim, "h"), "h");
    assert!(vim.disabled_bindings.is_empty());
}

#[test]
fn builtin_vim_binding_editor_remaps_ctrl_binding() {
    let mut vim = EditorVimSettings::default();

    set_builtin_vim_binding(&mut vim, "<C-r>", "H".to_owned());

    assert_eq!(vim_builtin_effective_binding(&vim, "<C-r>"), "H");
    assert!(
        vim.disabled_bindings
            .iter()
            .any(|binding| binding == "<C-r>")
    );
    assert!(vim.key_overrides.iter().any(|binding| {
        binding.before == "H" && binding.after == "<C-r>" && binding.command.is_none()
    }));

    reset_builtin_vim_binding(&mut vim, "<C-r>");
    assert_eq!(vim_builtin_effective_binding(&vim, "<C-r>"), "<C-r>");
    assert!(vim.disabled_bindings.is_empty());
    assert!(vim.key_overrides.is_empty());
}
