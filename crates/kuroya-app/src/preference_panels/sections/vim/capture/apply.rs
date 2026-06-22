use super::state::{VimKeyCaptureOriginal, VimKeyCaptureTarget};
use crate::preference_panels::sections::vim::{
    VimBindingOwner,
    builtins::vim_builtin_bindings,
    editing::{
        set_builtin_vim_binding, vim_after_sequence_error, vim_binding_edit_error,
        vim_disabled_binding_edit_error,
    },
};
use kuroya_core::EditorVimSettings;

pub(in crate::preference_panels::sections::vim) fn restore_vim_key_capture_original(
    vim: &mut EditorVimSettings,
    original: &VimKeyCaptureOriginal,
) {
    match original.target {
        VimKeyCaptureTarget::BuiltIn(default_binding) => {
            set_builtin_vim_binding(vim, default_binding, original.value.clone());
        }
        VimKeyCaptureTarget::CustomDisabled(index) => {
            if let Some(binding) = vim.disabled_bindings.get_mut(index) {
                *binding = original.value.clone();
            }
        }
        VimKeyCaptureTarget::CustomOverrideBefore(index) => {
            if let Some(binding) = vim.key_overrides.get_mut(index) {
                binding.before = original.value.clone();
            }
        }
        VimKeyCaptureTarget::CustomOverrideAfter(index) => {
            if let Some(binding) = vim.key_overrides.get_mut(index)
                && binding.command.is_none()
            {
                binding.after = original.value.clone();
            }
        }
    }
}

pub(in crate::preference_panels::sections::vim) fn apply_captured_vim_key(
    vim: &mut EditorVimSettings,
    target: VimKeyCaptureTarget,
    key: String,
) -> Result<(), String> {
    match target {
        VimKeyCaptureTarget::BuiltIn(default_binding) => apply_captured_vim_binding(
            vim,
            key,
            VimBindingOwner::BuiltIn(default_binding),
            |vim, key| set_builtin_vim_binding(vim, default_binding, key),
        ),
        VimKeyCaptureTarget::CustomDisabled(index) => {
            if index >= vim.disabled_bindings.len() {
                return Err("That disabled binding was removed".to_owned());
            };
            if let Some(message) =
                vim_disabled_binding_edit_error(&key, VimBindingOwner::CustomDisabled(index), vim)
            {
                Err(message)
            } else {
                vim.disabled_bindings[index] = key;
                Ok(())
            }
        }
        VimKeyCaptureTarget::CustomOverrideBefore(index) => apply_captured_vim_binding(
            vim,
            key,
            VimBindingOwner::CustomOverride(index),
            |vim, key| {
                if let Some(binding) = vim.key_overrides.get_mut(index) {
                    binding.before = key;
                }
            },
        ),
        VimKeyCaptureTarget::CustomOverrideAfter(index) => {
            let Some(binding) = vim.key_overrides.get_mut(index) else {
                return Err("That Vim override was removed".to_owned());
            };
            if binding.command.is_some() {
                return Err("Switch the target to Vim keys before capturing After".to_owned());
            }
            if let Some(message) = vim_after_sequence_error(&key) {
                Err(message)
            } else {
                binding.after = key;
                Ok(())
            }
        }
    }
}

fn apply_captured_vim_binding(
    vim: &mut EditorVimSettings,
    key: String,
    owner: VimBindingOwner,
    apply: impl FnOnce(&mut EditorVimSettings, String),
) -> Result<(), String> {
    match vim_binding_edit_error(&key, owner, vim) {
        Some(message) => Err(message),
        None => {
            apply(vim, key);
            Ok(())
        }
    }
}

pub(in crate::preference_panels::sections::vim) fn vim_key_capture_target_exists(
    target: VimKeyCaptureTarget,
    vim: &EditorVimSettings,
) -> bool {
    match target {
        VimKeyCaptureTarget::BuiltIn(default_binding) => {
            vim_builtin_bindings().any(|binding| binding.default == default_binding)
        }
        VimKeyCaptureTarget::CustomDisabled(index) => index < vim.disabled_bindings.len(),
        VimKeyCaptureTarget::CustomOverrideBefore(index) => index < vim.key_overrides.len(),
        VimKeyCaptureTarget::CustomOverrideAfter(index) => vim
            .key_overrides
            .get(index)
            .is_some_and(|binding| binding.command.is_none()),
    }
}
