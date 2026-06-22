use eframe::egui::{Key, Modifiers};

use super::super::super::super::{
    EditorVimNamedRegister, EditorVimPendingKey, vim_escape_key, vim_named_register_for_key,
};
use super::super::super::keys::{
    vim_visual_character_change_key, vim_visual_character_delete_key, vim_visual_character_yank_key,
};

pub(super) fn handle_visual_register_prefix_substate(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
) -> Option<Option<EditorVimPendingKey>> {
    if vim_escape_key(key, modifiers) {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if let Some(register) = vim_named_register_for_key(key, modifiers) {
        return Some(Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor,
            cursor,
            count,
            register,
        }));
    }
    printable_key_char.is_some().then_some(pending)
}

pub(super) fn handle_visual_register_command_substate(
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    register: EditorVimNamedRegister,
) -> Option<Option<EditorVimPendingKey>> {
    if vim_escape_key(key, modifiers)
        || vim_visual_character_yank_key(key, modifiers)
        || vim_visual_character_delete_key(key, modifiers)
        || vim_visual_character_change_key(key, modifiers)
    {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    printable_key_char.is_some().then_some(Some(
        EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor,
            cursor,
            count,
            register,
        },
    ))
}
