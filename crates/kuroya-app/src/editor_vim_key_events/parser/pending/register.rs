use eframe::egui::{Key, Modifiers};

use super::super::super::{EditorVimPendingKey, vim_named_register_for_key};
use super::super::{vim_register_command_count, vim_register_command_next_count};

pub(in crate::editor_vim_key_events) fn vim_pending_key_next_named_register(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    match pending {
        Some(EditorVimPendingKey::RegisterPrefix(count)) => {
            vim_named_register_for_key(key, modifiers).map(|register| {
                EditorVimPendingKey::RegisterCommand {
                    prefix_count: count,
                    command_count: None,
                    register,
                }
            })
        }
        Some(EditorVimPendingKey::RegisterCommand {
            prefix_count,
            command_count,
            register,
        }) => {
            if let Some(command_count) =
                vim_register_command_next_count(command_count, key, modifiers)
            {
                return Some(EditorVimPendingKey::RegisterCommand {
                    prefix_count,
                    command_count: Some(command_count),
                    register,
                });
            }
            let count = vim_register_command_count(prefix_count, command_count);
            if modifiers.command || modifiers.alt || modifiers.ctrl || modifiers.shift {
                return None;
            }
            match key {
                Key::C => Some(EditorVimPendingKey::ChangeLineIntoRegister {
                    operator_count: count,
                    register,
                }),
                Key::D => Some(EditorVimPendingKey::DeleteLineIntoRegister {
                    operator_count: count,
                    register,
                }),
                Key::Y => Some(EditorVimPendingKey::YankLineIntoRegister {
                    operator_count: count,
                    register,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}
