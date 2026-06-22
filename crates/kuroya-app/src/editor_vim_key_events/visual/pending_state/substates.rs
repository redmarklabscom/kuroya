use eframe::egui::{Key, Modifiers};

mod register;

use self::register::{
    handle_visual_register_command_substate, handle_visual_register_prefix_substate,
};
use super::super::super::{EditorVimPendingKey, vim_escape_key, vim_text_object_kind_for_key};

pub(super) struct VisualPendingBaseState {
    pub(super) anchor: usize,
    pub(super) cursor: usize,
    pub(super) count: Option<usize>,
}

pub(super) enum VisualPendingStateAfterKey {
    Base(VisualPendingBaseState),
    Resolved(Option<Option<EditorVimPendingKey>>),
}

pub(super) fn vim_visual_pending_substate_after_key(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
) -> VisualPendingStateAfterKey {
    match pending {
        Some(EditorVimPendingKey::VisualCharacter { anchor, cursor }) => {
            VisualPendingStateAfterKey::Base(VisualPendingBaseState {
                anchor,
                cursor,
                count: None,
            })
        }
        Some(EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count,
        }) => VisualPendingStateAfterKey::Base(VisualPendingBaseState {
            anchor,
            cursor,
            count: Some(count),
        }),
        Some(EditorVimPendingKey::VisualCharacterGo {
            anchor,
            cursor,
            count: _,
        }) => VisualPendingStateAfterKey::Resolved(handle_visual_go_substate(
            key,
            modifiers,
            printable_key_char,
            anchor,
            cursor,
        )),
        Some(EditorVimPendingKey::VisualCharacterTextObject {
            anchor,
            cursor,
            count,
            scope,
        }) => VisualPendingStateAfterKey::Resolved(handle_visual_text_object_substate(
            key,
            modifiers,
            printable_key_char,
            anchor,
            cursor,
            count,
            scope,
        )),
        Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
            anchor,
            cursor,
            count,
        }) => VisualPendingStateAfterKey::Resolved(handle_visual_register_prefix_substate(
            pending,
            key,
            modifiers,
            printable_key_char,
            anchor,
            cursor,
            count,
        )),
        Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor,
            cursor,
            count,
            register,
        }) => VisualPendingStateAfterKey::Resolved(handle_visual_register_command_substate(
            key,
            modifiers,
            printable_key_char,
            anchor,
            cursor,
            count,
            register,
        )),
        Some(EditorVimPendingKey::VisualCharacterReplace { .. }) => {
            VisualPendingStateAfterKey::Resolved(handle_visual_replace_substate(
                key,
                modifiers,
                printable_key_char,
            ))
        }
        Some(EditorVimPendingKey::VisualCharacterCharFind { anchor, cursor, .. }) => {
            VisualPendingStateAfterKey::Resolved(handle_visual_char_find_substate(
                key,
                modifiers,
                printable_key_char,
                anchor,
                cursor,
            ))
        }
        _ => VisualPendingStateAfterKey::Resolved(None),
    }
}

fn handle_visual_go_substate(
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
    anchor: usize,
    cursor: usize,
) -> Option<Option<EditorVimPendingKey>> {
    if vim_escape_key(key, modifiers) {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if matches!(
        (key, modifiers.shift),
        (Key::Num8, true) | (Key::Num3, true)
    ) {
        return Some(Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor,
        }));
    }
    printable_key_char
        .is_some()
        .then_some(Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor,
        }))
}

fn handle_visual_text_object_substate(
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    scope: super::super::super::EditorVimTextObjectScope,
) -> Option<Option<EditorVimPendingKey>> {
    if vim_escape_key(key, modifiers) {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if vim_text_object_kind_for_key(key, modifiers).is_some() {
        return Some(Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor,
        }));
    }
    printable_key_char
        .is_some()
        .then_some(Some(EditorVimPendingKey::VisualCharacterTextObject {
            anchor,
            cursor,
            count,
            scope,
        }))
}

fn handle_visual_replace_substate(
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
) -> Option<Option<EditorVimPendingKey>> {
    if vim_escape_key(key, modifiers) {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    printable_key_char.is_some().then_some(None)
}

fn handle_visual_char_find_substate(
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
    anchor: usize,
    cursor: usize,
) -> Option<Option<EditorVimPendingKey>> {
    if vim_escape_key(key, modifiers) {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    printable_key_char
        .is_some()
        .then_some(Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor,
        }))
}
