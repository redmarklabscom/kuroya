use eframe::egui::{Key, Modifiers};

use super::super::key_tokens::vim_key_sequence_events;
use super::super::{
    EditorVimMode, EditorVimPendingKey, insert_mode_key_can_mutate, vim_command_input_accept_key,
    vim_command_input_cancel_key, vim_command_input_control_edit, vim_escape_key,
    vim_normal_key_can_mutate, vim_normal_key_is_handled, vim_normal_key_next_mode,
    vim_normal_key_next_pending, vim_normal_key_next_pending_after_count, vim_pending_key_accepts,
    vim_pending_key_next_char_find, vim_pending_key_next_named_register,
    vim_pending_key_next_operator_count, vim_pending_key_next_operator_go,
    vim_pending_key_next_text_object, vim_printable_key_char, vim_search_input_accept_key,
    vim_search_input_cancel_key, vim_search_input_control_edit, vim_visual_pending_after_key,
};

pub(crate) fn vim_key_sequence_is_normal_mode_supported(sequence: &str) -> bool {
    let Some(keys) = vim_key_sequence_events(sequence.trim()) else {
        return false;
    };
    if keys.is_empty() {
        return false;
    }

    let mut mode = EditorVimMode::Normal;
    let mut pending = None;

    keys.into_iter().all(|(key, modifiers)| {
        vim_key_sequence_scan_step_is_supported(key, modifiers, &mut mode, &mut pending)
    })
}

fn vim_key_sequence_scan_step_is_supported(
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
) -> bool {
    match *mode {
        EditorVimMode::Insert => {
            if vim_escape_key(key, modifiers) {
                *mode = EditorVimMode::Normal;
                *pending = None;
                true
            } else {
                insert_mode_key_can_mutate(key, modifiers)
            }
        }
        EditorVimMode::Normal => {
            let printable_key_char = vim_printable_key_char(key, modifiers);
            if vim_normal_key_can_mutate(key, modifiers, *pending, printable_key_char) {
                *pending = None;
                return true;
            }
            if vim_escape_key(key, modifiers) {
                *pending = None;
                return true;
            }
            if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
                if vim_search_input_accept_key(key, modifiers)
                    || vim_search_input_cancel_key(key, modifiers)
                {
                    *pending = None;
                    return true;
                }
                return vim_search_input_control_edit(key, modifiers).is_some()
                    || printable_key_char.is_some();
            }
            if matches!(*pending, Some(EditorVimPendingKey::CommandInput)) {
                if vim_command_input_accept_key(key, modifiers)
                    || vim_command_input_cancel_key(key, modifiers)
                {
                    *pending = None;
                    return true;
                }
                return vim_command_input_control_edit(key, modifiers).is_some()
                    || printable_key_char.is_some();
            }
            if let Some(next_pending) =
                vim_visual_pending_after_key(*pending, key, modifiers, printable_key_char)
            {
                *pending = next_pending;
                true
            } else if let Some(next_pending) =
                vim_pending_key_next_named_register(*pending, key, modifiers)
            {
                *pending = Some(next_pending);
                true
            } else if let Some(next_pending) =
                vim_pending_key_next_operator_count(*pending, key, modifiers)
            {
                *pending = Some(next_pending);
                true
            } else if let Some(next_pending) =
                vim_pending_key_next_operator_go(*pending, key, modifiers)
            {
                *pending = Some(next_pending);
                true
            } else if let Some(next_pending) =
                vim_pending_key_next_text_object(*pending, key, modifiers)
            {
                *pending = Some(next_pending);
                true
            } else if let Some(next_pending) =
                vim_pending_key_next_char_find(*pending, key, modifiers)
            {
                *pending = Some(next_pending);
                true
            } else if !matches!(*pending, Some(EditorVimPendingKey::Count(_)))
                && vim_pending_key_accepts(*pending, key, modifiers, printable_key_char)
            {
                *pending = None;
                true
            } else if let Some(EditorVimPendingKey::Count(count)) = *pending
                && let Some(next_pending) =
                    vim_normal_key_next_pending_after_count(count, key, modifiers)
            {
                *pending = Some(next_pending);
                true
            } else if let Some(next_pending) = vim_normal_key_next_pending(key, modifiers) {
                *pending = Some(next_pending);
                true
            } else if let Some(next_mode) = vim_normal_key_next_mode(key, modifiers) {
                *pending = None;
                *mode = next_mode;
                true
            } else if vim_normal_key_is_handled(key, modifiers) {
                if !vim_pending_key_accepts(*pending, key, modifiers, printable_key_char) {
                    *pending = None;
                }
                true
            } else {
                *pending = None;
                false
            }
        }
    }
}
