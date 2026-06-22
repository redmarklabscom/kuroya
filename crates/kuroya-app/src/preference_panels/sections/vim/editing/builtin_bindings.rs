use crate::editor_vim_key_events::vim_key_sequences_match;
use crate::preference_panels::sections::vim::builtins::vim_builtin_bindings;
use kuroya_core::{EditorVimKeyOverride, EditorVimSettings};

pub(in crate::preference_panels::sections::vim) fn vim_builtin_effective_binding(
    vim: &EditorVimSettings,
    default_binding: &str,
) -> String {
    if vim_builtin_default_disabled(vim, default_binding) {
        if let Some(binding) = vim.key_overrides.iter().find(|binding| {
            binding.command.is_none() && vim_key_sequences_match(&binding.after, default_binding)
        }) {
            return binding.before.clone();
        }
        String::new()
    } else {
        default_binding.to_owned()
    }
}

pub(in crate::preference_panels::sections::vim) fn set_builtin_vim_binding(
    vim: &mut EditorVimSettings,
    default_binding: &'static str,
    candidate: String,
) {
    remove_builtin_vim_override(vim, default_binding);
    remove_disabled_vim_binding(vim, default_binding);

    if candidate.is_empty() {
        push_disabled_vim_binding(vim, default_binding);
    } else if !vim_key_sequences_match(&candidate, default_binding) {
        remove_disabled_vim_binding(vim, &candidate);
        push_disabled_vim_binding(vim, default_binding);
        insert_builtin_vim_override(
            vim,
            EditorVimKeyOverride {
                before: candidate,
                after: default_binding.to_owned(),
                command: None,
            },
        );
    }

    vim.sanitize();
}

pub(in crate::preference_panels::sections::vim) fn reset_builtin_vim_binding(
    vim: &mut EditorVimSettings,
    default_binding: &'static str,
) {
    remove_builtin_vim_override(vim, default_binding);
    remove_disabled_vim_binding(vim, default_binding);
    vim.sanitize();
}

pub(in crate::preference_panels::sections::vim) fn disable_builtin_vim_binding(
    vim: &mut EditorVimSettings,
    default_binding: &'static str,
) {
    remove_builtin_vim_override(vim, default_binding);
    push_disabled_vim_binding(vim, default_binding);
    vim.sanitize();
}

pub(in crate::preference_panels::sections::vim) fn binding_is_builtin_override(
    index: usize,
    binding: &EditorVimKeyOverride,
    vim: &EditorVimSettings,
) -> bool {
    binding.command.is_none()
        && vim_builtin_bindings().any(|builtin| {
            builtin_vim_override_index(vim, builtin.default) == Some(index)
                && vim_key_sequences_match(&binding.after, builtin.default)
        })
}

pub(in crate::preference_panels::sections::vim) fn binding_is_builtin_default(
    binding: &str,
) -> bool {
    vim_builtin_bindings().any(|builtin| vim_key_sequences_match(binding, builtin.default))
}

fn remove_builtin_vim_override(vim: &mut EditorVimSettings, default_binding: &str) {
    if let Some(index) = builtin_vim_override_index(vim, default_binding) {
        vim.key_overrides.remove(index);
    }
}

fn push_disabled_vim_binding(vim: &mut EditorVimSettings, binding: &str) {
    if !vim
        .disabled_bindings
        .iter()
        .any(|existing| vim_key_sequences_match(existing, binding))
    {
        vim.disabled_bindings.push(binding.to_owned());
    }
}

fn remove_disabled_vim_binding(vim: &mut EditorVimSettings, binding: &str) {
    vim.disabled_bindings
        .retain(|existing| !vim_key_sequences_match(existing, binding));
}

fn vim_builtin_default_disabled(vim: &EditorVimSettings, default_binding: &str) -> bool {
    vim.disabled_bindings
        .iter()
        .any(|binding| vim_key_sequences_match(binding, default_binding))
}

fn builtin_vim_override_index(vim: &EditorVimSettings, default_binding: &str) -> Option<usize> {
    vim_builtin_default_disabled(vim, default_binding).then(|| {
        vim.key_overrides.iter().position(|binding| {
            binding.command.is_none() && vim_key_sequences_match(&binding.after, default_binding)
        })
    })?
}

fn insert_builtin_vim_override(
    vim: &mut EditorVimSettings,
    override_binding: EditorVimKeyOverride,
) {
    let insert_at = vim
        .key_overrides
        .iter()
        .position(|binding| {
            binding.command.is_none()
                && vim_key_sequences_match(&binding.after, &override_binding.after)
        })
        .unwrap_or(vim.key_overrides.len());
    vim.key_overrides.insert(insert_at, override_binding);
}
