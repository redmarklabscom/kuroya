use eframe::egui::{Key, Modifiers};

use super::super::super::EditorVimPendingKey;
use super::super::vim_text_object_scope_for_key;

pub(in crate::editor_vim_key_events) fn vim_pending_key_next_text_object(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    let scope = vim_text_object_scope_for_key(key, modifiers)?;
    match pending {
        Some(EditorVimPendingKey::ChangeLine(operator_count)) => {
            Some(EditorVimPendingKey::ChangeTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeTextObjectIntoRegister {
            operator_count,
            motion_count: 1,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::DeleteLine(operator_count)) => {
            Some(EditorVimPendingKey::DeleteTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteTextObjectIntoRegister {
            operator_count,
            motion_count: 1,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::YankLine(operator_count)) => {
            Some(EditorVimPendingKey::YankTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::YankTextObjectIntoRegister {
            operator_count,
            motion_count: 1,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ChangeTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::DeleteTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::YankTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        Some(EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::YankTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseTextObject {
            operator_count,
            motion_count: 1,
            scope,
            conversion,
        }),
        Some(EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseTextObject {
            operator_count,
            motion_count,
            scope,
            conversion,
        }),
        Some(EditorVimPendingKey::ToggleCaseOperator(operator_count)) => {
            Some(EditorVimPendingKey::ToggleCaseTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ToggleCaseTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        _ => None,
    }
}
