use super::{
    builtin_bindings::vim_builtin_effective_binding,
    custom_overrides::{custom_disabled_binding_indices, custom_override_indices},
};
use crate::{
    commands::command_label,
    editor_vim_key_events::{
        vim_key_sequence_is_single_supported, vim_key_sequence_is_supported,
        vim_key_sequence_starts_with, vim_key_sequences_match,
    },
    preference_panels::sections::{
        bounded_settings_singleline_input, vim::VimBindingOwner,
        vim::builtins::vim_builtin_bindings,
    },
};
use kuroya_core::{EditorVimKeyOverride, EditorVimSettings};

pub(in crate::preference_panels::sections::vim) fn vim_binding_existing_error(
    binding: &str,
    owner: VimBindingOwner,
    vim: &EditorVimSettings,
) -> Option<String> {
    let candidate = normalized_vim_binding_edit(binding);
    if candidate.is_empty() {
        return None;
    }
    vim_binding_edit_error(&candidate, owner, vim)
}

pub(in crate::preference_panels::sections::vim) fn vim_binding_edit_error(
    candidate: &str,
    owner: VimBindingOwner,
    vim: &EditorVimSettings,
) -> Option<String> {
    if candidate.is_empty() {
        return None;
    }
    if !vim_key_sequence_is_single_supported(candidate) {
        return Some("Use one supported Vim key".to_owned());
    }
    vim_binding_conflict_owner(candidate, owner, vim)
        .map(|owner| format!("Already used by {owner}"))
}

pub(in crate::preference_panels::sections::vim) fn vim_override_before_edit_error(
    candidate: &str,
    owner: VimBindingOwner,
    vim: &EditorVimSettings,
) -> Option<String> {
    if candidate.is_empty() {
        return None;
    }
    if !vim_key_sequence_is_supported(candidate) {
        return Some("Use supported Vim keys".to_owned());
    }
    vim_binding_conflict_owner(candidate, owner, vim)
        .map(|owner| format!("Already used by {owner}"))
}

fn vim_binding_conflict_owner(
    candidate: &str,
    owner: VimBindingOwner,
    vim: &EditorVimSettings,
) -> Option<String> {
    for binding in vim_builtin_bindings() {
        if matches!(owner, VimBindingOwner::BuiltIn(default) if default == binding.default) {
            continue;
        }
        let effective = vim_builtin_effective_binding(vim, binding.default);
        if !effective.is_empty() && vim_key_sequences_match(candidate, &effective) {
            return Some(binding.label.to_owned());
        }
    }

    for (index, binding) in vim.key_overrides.iter().enumerate() {
        if matches!(owner, VimBindingOwner::CustomOverride(owner_index) if owner_index == index) {
            continue;
        }
        if matches!(owner, VimBindingOwner::BuiltIn(default) if binding.command.is_none() && vim_key_sequences_match(&binding.after, default))
        {
            continue;
        }
        if !binding.before.is_empty() && vim_key_bindings_conflict(candidate, &binding.before) {
            return Some(vim_override_owner_label(index, binding, vim));
        }
    }

    for (index, binding) in vim.disabled_bindings.iter().enumerate() {
        if matches!(owner, VimBindingOwner::CustomDisabled(owner_index) if owner_index == index) {
            continue;
        }
        if matches!(owner, VimBindingOwner::BuiltIn(default) if vim_key_sequences_match(binding, default))
        {
            continue;
        }
        if vim_key_bindings_conflict(candidate, binding) {
            return Some(format!(
                "disabled binding {}",
                custom_disabled_binding_visible_number(index, vim)
            ));
        }
    }

    None
}

fn vim_key_bindings_conflict(left: &str, right: &str) -> bool {
    vim_key_sequences_match(left, right)
        || vim_key_sequence_starts_with(left, right)
        || vim_key_sequence_starts_with(right, left)
}

fn vim_override_owner_label(
    index: usize,
    binding: &EditorVimKeyOverride,
    vim: &EditorVimSettings,
) -> String {
    let visible_number = custom_override_visible_number(index, vim);
    match &binding.command {
        Some(command) => format!(
            "custom command {visible_number} ({})",
            command_label(command)
        ),
        None => format!("custom remap {visible_number} ({})", binding.after),
    }
}

fn custom_override_visible_number(index: usize, vim: &EditorVimSettings) -> usize {
    custom_override_indices(vim)
        .into_iter()
        .position(|visible_index| visible_index == index)
        .map(|visible_index| visible_index + 1)
        .unwrap_or(index + 1)
}

fn custom_disabled_binding_visible_number(index: usize, vim: &EditorVimSettings) -> usize {
    custom_disabled_binding_indices(vim)
        .into_iter()
        .position(|visible_index| visible_index == index)
        .map(|visible_index| visible_index + 1)
        .unwrap_or(index + 1)
}

pub(in crate::preference_panels::sections::vim) fn vim_disabled_binding_edit_error(
    binding: &str,
    owner: VimBindingOwner,
    vim: &EditorVimSettings,
) -> Option<String> {
    let candidate = normalized_vim_binding_edit(binding);
    if candidate.is_empty() {
        return None;
    }
    if !vim_key_sequence_is_single_supported(&candidate) {
        return Some("Use one supported Vim key".to_owned());
    }
    vim_binding_conflict_owner(&candidate, owner, vim)
        .map(|owner| format!("Already used by {owner}"))
}

pub(in crate::preference_panels::sections::vim) fn normalized_vim_binding_edit(
    value: &str,
) -> String {
    bounded_settings_singleline_input(value).trim().to_owned()
}
