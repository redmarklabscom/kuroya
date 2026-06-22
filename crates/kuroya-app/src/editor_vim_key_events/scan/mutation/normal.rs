use eframe::egui::{Key, Modifiers};
use kuroya_core::EditorVimSettings;
use std::collections::VecDeque;

use super::super::super::settings_overrides::vim_command_override_can_mutate;
use super::super::super::{
    EditorVimMode, EditorVimPendingKey, VimSettingsPreflightAction, vim_command_input_accept_key,
    vim_command_input_cancel_key, vim_command_input_control_edit, vim_escape_key,
    vim_normal_key_can_mutate, vim_normal_key_is_handled, vim_normal_key_next_mode,
    vim_normal_key_next_pending, vim_normal_key_next_pending_after_count, vim_pending_key_accepts,
    vim_pending_key_next_char_find, vim_pending_key_next_named_register,
    vim_pending_key_next_operator_count, vim_pending_key_next_operator_go,
    vim_pending_key_next_text_object, vim_printable_key_char, vim_search_input_accept_key,
    vim_search_input_cancel_key, vim_search_input_control_edit, vim_settings_preflight_action,
    vim_visual_pending_after_key,
};
use super::super::suppression::vim_suppress_printable_key_text_if;

pub(super) fn vim_normal_key_event_includes_mutation_for_scan(
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    suppressed_text: &mut VecDeque<char>,
    vim_settings: Option<&EditorVimSettings>,
    suppress_text_echo: bool,
) -> bool {
    let settings_action = if let Some(settings) = vim_settings
        && !matches!(
            *pending,
            Some(EditorVimPendingKey::CommandInput | EditorVimPendingKey::SearchInput { .. })
        ) {
        let allow_command_override = pending.is_none();
        vim_settings_preflight_action(key, modifiers, settings, pending, allow_command_override)
    } else {
        None
    };
    if let Some(action) = settings_action {
        vim_suppress_printable_key_text_if(
            vim_printable_key_char(key, modifiers),
            suppressed_text,
            suppress_text_echo,
        );
        return match action {
            VimSettingsPreflightAction::Handled => false,
            VimSettingsPreflightAction::Command(command) => {
                vim_command_override_can_mutate(&command)
            }
            VimSettingsPreflightAction::Remap(keys) => keys.into_iter().any(|(key, modifiers)| {
                super::vim_key_event_includes_mutation_for_scan(
                    key,
                    modifiers,
                    mode,
                    pending,
                    suppressed_text,
                    None,
                    false,
                )
            }),
        };
    }

    let printable_key_char = vim_printable_key_char(key, modifiers);
    if vim_normal_key_can_mutate(key, modifiers, *pending, printable_key_char) {
        return true;
    }
    if vim_escape_key(key, modifiers) {
        *pending = None;
        return false;
    }
    if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
        if vim_search_input_accept_key(key, modifiers) {
            *pending = None;
            return false;
        }
        if vim_search_input_cancel_key(key, modifiers) {
            *pending = None;
            return false;
        }
        if vim_search_input_control_edit(key, modifiers).is_some() {
            return false;
        }
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        return false;
    }
    if matches!(*pending, Some(EditorVimPendingKey::CommandInput)) {
        if vim_command_input_accept_key(key, modifiers) {
            *pending = None;
            return false;
        }
        if vim_command_input_cancel_key(key, modifiers) {
            *pending = None;
            return false;
        }
        if vim_command_input_control_edit(key, modifiers).is_some() {
            return false;
        }
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        return false;
    }
    if let Some(next_pending) =
        vim_visual_pending_after_key(*pending, key, modifiers, printable_key_char)
    {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = next_pending;
    } else if let Some(next_pending) = vim_pending_key_next_named_register(*pending, key, modifiers)
    {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if let Some(next_pending) = vim_pending_key_next_operator_count(*pending, key, modifiers)
    {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if let Some(next_pending) = vim_pending_key_next_operator_go(*pending, key, modifiers) {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if let Some(next_pending) = vim_pending_key_next_text_object(*pending, key, modifiers) {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if let Some(next_pending) = vim_pending_key_next_char_find(*pending, key, modifiers) {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if !matches!(*pending, Some(EditorVimPendingKey::Count(_)))
        && vim_pending_key_accepts(*pending, key, modifiers, printable_key_char)
    {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = None;
    } else if let Some(EditorVimPendingKey::Count(count)) = *pending
        && let Some(next_pending) = vim_normal_key_next_pending_after_count(count, key, modifiers)
    {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if let Some(next_pending) = vim_normal_key_next_pending(key, modifiers) {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = Some(next_pending);
    } else if let Some(next_mode) = vim_normal_key_next_mode(key, modifiers) {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        *pending = None;
        *mode = next_mode;
    } else if vim_normal_key_is_handled(key, modifiers) {
        vim_suppress_printable_key_text_if(printable_key_char, suppressed_text, suppress_text_echo);
        if !vim_pending_key_accepts(*pending, key, modifiers, printable_key_char) {
            *pending = None;
        }
    } else {
        *pending = None;
    }
    false
}
