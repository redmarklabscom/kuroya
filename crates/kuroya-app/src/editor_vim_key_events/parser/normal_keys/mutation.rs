mod direct;
mod operator_pending;
mod typed_pending;
mod visual_pending;

use eframe::egui::{Key, Modifiers};

use self::direct::vim_direct_normal_key_can_mutate;
use self::operator_pending::vim_operator_pending_key_can_mutate;
use self::typed_pending::vim_typed_pending_key_can_mutate;
use self::visual_pending::vim_visual_pending_key_can_mutate;
use super::super::super::EditorVimPendingKey;

pub(in crate::editor_vim_key_events) fn vim_normal_key_can_mutate(
    key: Key,
    modifiers: Modifiers,
    pending: Option<EditorVimPendingKey>,
    printable_key_char: Option<char>,
) -> bool {
    if modifiers.command || modifiers.alt {
        return false;
    }
    if modifiers.ctrl {
        return key == Key::R && !modifiers.shift;
    }
    if let Some(can_mutate) =
        vim_visual_pending_key_can_mutate(key, modifiers, pending, printable_key_char)
    {
        return can_mutate;
    }
    if let Some(can_mutate) =
        vim_typed_pending_key_can_mutate(key, modifiers, pending, printable_key_char)
    {
        return can_mutate;
    }
    if let Some(can_mutate) = vim_operator_pending_key_can_mutate(key, modifiers, pending) {
        return can_mutate;
    }
    vim_direct_normal_key_can_mutate(key, modifiers)
}
