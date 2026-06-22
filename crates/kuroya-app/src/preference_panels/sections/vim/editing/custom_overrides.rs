use super::{
    builtin_bindings::{binding_is_builtin_default, binding_is_builtin_override},
    validation::{
        normalized_vim_binding_edit, vim_disabled_binding_edit_error,
        vim_override_before_edit_error,
    },
};
use crate::editor_vim_key_events::vim_key_sequence_is_normal_mode_supported;
use crate::preference_panels::sections::vim::VimBindingOwner;
use kuroya_core::{Command, EditorVimKeyOverride, EditorVimSettings};

const CUSTOM_VIM_BINDING_CANDIDATES: &[&str] =
    &["q", "z", "Q", "Z", "<Left>", "<Right>", "<Up>", "<Down>"];

pub(in crate::preference_panels::sections::vim) fn switch_vim_override_to_keys(
    binding: &mut EditorVimKeyOverride,
) {
    binding.command = None;
    let candidate = normalized_vim_binding_edit(&binding.after);
    if candidate.is_empty() || !vim_key_sequence_is_normal_mode_supported(&candidate) {
        binding.after = default_vim_override_after().to_owned();
    } else {
        binding.after = candidate;
    }
}

pub(in crate::preference_panels::sections::vim) fn vim_after_sequence_error(
    sequence: &str,
) -> Option<String> {
    let candidate = normalized_vim_binding_edit(sequence);
    if candidate.is_empty() {
        Some("After must contain Vim keys".to_owned())
    } else if vim_key_sequence_is_normal_mode_supported(&candidate) {
        None
    } else {
        Some("Use supported Vim keys".to_owned())
    }
}

pub(in crate::preference_panels::sections::vim) fn apply_custom_override_after_edit(
    vim: &mut EditorVimSettings,
    index: usize,
    value: &str,
) -> Option<String> {
    let candidate = normalized_vim_binding_edit(value);
    let error = vim_after_sequence_error(&candidate);
    if error.is_none()
        && let Some(binding) = vim.key_overrides.get_mut(index)
        && binding.command.is_none()
    {
        binding.after = candidate;
    }
    error
}

pub(in crate::preference_panels::sections::vim) fn custom_disabled_binding_indices(
    vim: &EditorVimSettings,
) -> Vec<usize> {
    vim.disabled_bindings
        .iter()
        .enumerate()
        .filter_map(|(index, binding)| {
            (!binding_is_builtin_default(binding) || binding.trim().is_empty()).then_some(index)
        })
        .collect()
}

pub(in crate::preference_panels::sections::vim) fn custom_override_indices(
    vim: &EditorVimSettings,
) -> Vec<usize> {
    vim.key_overrides
        .iter()
        .enumerate()
        .filter_map(|(index, binding)| {
            (!binding_is_builtin_override(index, binding, vim)).then_some(index)
        })
        .collect()
}

pub(in crate::preference_panels::sections::vim) fn default_vim_override_command() -> Command {
    Command::RequestHover
}

pub(in crate::preference_panels::sections::vim) fn default_custom_disabled_vim_binding(
    vim: &EditorVimSettings,
) -> Option<String> {
    default_custom_vim_binding(|candidate| {
        vim_disabled_binding_edit_error(candidate, VimBindingOwner::CustomDisabled(usize::MAX), vim)
    })
}

fn default_custom_override_before(vim: &EditorVimSettings) -> Option<String> {
    default_custom_vim_binding(|candidate| {
        vim_override_before_edit_error(candidate, VimBindingOwner::CustomOverride(usize::MAX), vim)
    })
}

fn default_custom_vim_binding(error_for: impl Fn(&str) -> Option<String>) -> Option<String> {
    CUSTOM_VIM_BINDING_CANDIDATES
        .iter()
        .copied()
        .find(|candidate| error_for(candidate).is_none())
        .map(ToOwned::to_owned)
}

fn default_vim_override_after() -> &'static str {
    "0"
}

pub(in crate::preference_panels::sections::vim) fn vim_key_override_to_keys(
    vim: &EditorVimSettings,
) -> Option<EditorVimKeyOverride> {
    Some(EditorVimKeyOverride {
        before: default_custom_override_before(vim)?,
        after: default_vim_override_after().to_owned(),
        command: None,
    })
}

pub(in crate::preference_panels::sections::vim) fn command_vim_key_override(
    vim: &EditorVimSettings,
) -> Option<EditorVimKeyOverride> {
    Some(EditorVimKeyOverride {
        before: default_custom_override_before(vim)?,
        after: String::new(),
        command: Some(default_vim_override_command()),
    })
}
