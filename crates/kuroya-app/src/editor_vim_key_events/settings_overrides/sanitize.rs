use kuroya_core::{EditorVimKeyOverride, EditorVimSettings};

use super::super::key_tokens::{
    vim_key_sequence_is_single_supported, vim_key_sequence_is_supported,
    vim_key_sequence_starts_with, vim_key_sequences_match,
};
use super::super::vim_key_sequence_is_normal_mode_supported;

pub(crate) fn sanitize_vim_settings_for_runtime(vim: &mut EditorVimSettings) -> bool {
    let mut changed = sanitize_vim_disabled_bindings_for_runtime(&mut vim.disabled_bindings);

    let original = std::mem::take(&mut vim.key_overrides);
    let mut normalized: Vec<EditorVimKeyOverride> = Vec::with_capacity(original.len());
    for binding in original {
        if vim_key_override_supported_by_runtime(&binding)
            && !vim_disabled_bindings_block_override(&vim.disabled_bindings, &binding.before)
            && !normalized.iter().any(|kept: &EditorVimKeyOverride| {
                vim_key_sequences_match(&kept.before, &binding.before)
            })
        {
            normalized.push(binding);
        } else {
            changed = true;
        }
    }
    vim.key_overrides = normalized;
    changed
}

fn sanitize_vim_disabled_bindings_for_runtime(bindings: &mut Vec<String>) -> bool {
    let original = std::mem::take(bindings);
    let mut normalized: Vec<String> = Vec::with_capacity(original.len());
    let mut changed = false;
    for binding in original {
        if !vim_key_sequence_is_single_supported(&binding) {
            changed = true;
            continue;
        }
        if normalized
            .iter()
            .any(|kept| vim_key_sequences_match(kept, &binding))
        {
            changed = true;
            continue;
        }
        normalized.push(binding);
    }
    *bindings = normalized;
    changed
}

fn vim_disabled_bindings_block_override(disabled_bindings: &[String], binding: &str) -> bool {
    disabled_bindings
        .iter()
        .any(|disabled| vim_key_sequence_starts_with(binding, disabled))
}

fn vim_key_override_supported_by_runtime(binding: &EditorVimKeyOverride) -> bool {
    if !vim_key_sequence_is_supported(&binding.before) {
        return false;
    }
    binding.command.is_some() || vim_key_sequence_is_normal_mode_supported(&binding.after)
}
