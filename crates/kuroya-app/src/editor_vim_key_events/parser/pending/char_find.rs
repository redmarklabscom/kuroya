use eframe::egui::{Key, Modifiers};

use super::super::super::EditorVimPendingKey;
use super::super::vim_operator_char_find_motion_for_key;

pub(in crate::editor_vim_key_events) fn vim_pending_key_next_char_find(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    let motion = vim_operator_char_find_motion_for_key(key, modifiers)?;
    match pending {
        Some(EditorVimPendingKey::ChangeLine(operator_count)) => {
            Some(EditorVimPendingKey::ChangeCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeCharFindIntoRegister {
            operator_count,
            motion_count: 1,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::DeleteLine(operator_count)) => {
            Some(EditorVimPendingKey::DeleteCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteCharFindIntoRegister {
            operator_count,
            motion_count: 1,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::YankLine(operator_count)) => {
            Some(EditorVimPendingKey::YankCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::YankCharFindIntoRegister {
            operator_count,
            motion_count: 1,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::ToggleCaseOperator(operator_count)) => {
            Some(EditorVimPendingKey::ToggleCaseCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseCharFind {
            operator_count,
            motion_count: 1,
            motion,
            conversion,
        }),
        Some(EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ChangeCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::DeleteCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::YankCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::YankCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ToggleCaseCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseCharFind {
            operator_count,
            motion_count,
            motion,
            conversion,
        }),
        _ => None,
    }
}
