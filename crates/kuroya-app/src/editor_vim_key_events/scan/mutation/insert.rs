use eframe::egui::{Key, Modifiers};
use kuroya_core::EditorVimSettings;
use std::collections::VecDeque;

use super::super::super::settings_overrides::vim_command_override_can_mutate;
use super::super::super::{
    EditorVimMode, EditorVimPendingKey, VimSettingsPreflightAction, insert_mode_key_can_mutate,
    vim_escape_key, vim_settings_preflight_action,
};

pub(super) fn vim_insert_key_event_includes_mutation_for_scan(
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    suppressed_text: &mut VecDeque<char>,
    vim_settings: Option<&EditorVimSettings>,
) -> bool {
    if vim_escape_key(key, modifiers) {
        if let Some(settings) = vim_settings
            && let Some(action) =
                vim_settings_preflight_action(key, modifiers, settings, pending, true)
        {
            return match action {
                VimSettingsPreflightAction::Handled => false,
                VimSettingsPreflightAction::Command(command) => {
                    vim_command_override_can_mutate(&command)
                }
                VimSettingsPreflightAction::Remap(keys) => {
                    *mode = EditorVimMode::Normal;
                    *pending = None;
                    keys.into_iter().any(|(key, modifiers)| {
                        super::vim_key_event_includes_mutation_for_scan(
                            key,
                            modifiers,
                            mode,
                            pending,
                            suppressed_text,
                            None,
                            false,
                        )
                    })
                }
            };
        }
        *mode = EditorVimMode::Normal;
        *pending = None;
    } else if let Some(settings) = vim_settings
        && let Some(VimSettingsPreflightAction::Remap(keys)) =
            vim_settings_preflight_action(key, modifiers, settings, pending, false)
        && keys.as_slice() == [(Key::Escape, Modifiers::NONE)]
    {
        *mode = EditorVimMode::Normal;
        *pending = None;
    } else if insert_mode_key_can_mutate(key, modifiers) {
        return true;
    }
    false
}
