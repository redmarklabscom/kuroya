use eframe::egui::{Key, Modifiers};

use super::super::super::{EditorVimOperatorGoKind, EditorVimPendingKey};

pub(in crate::editor_vim_key_events) fn vim_pending_key_next_operator_go(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    if key != Key::G || modifiers.shift || modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    let (operator_count, motion_count, operator) = match pending? {
        EditorVimPendingKey::ChangeLine(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::Change)
        }
        EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::ChangeIntoRegister(register),
        ),
        EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::Change,
        ),
        EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::ChangeIntoRegister(register),
        ),
        EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::ConvertCase(conversion),
        ),
        EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::ConvertCase(conversion),
        ),
        EditorVimPendingKey::DeleteLine(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::Delete)
        }
        EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::DeleteIntoRegister(register),
        ),
        EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::Delete,
        ),
        EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::DeleteIntoRegister(register),
        ),
        EditorVimPendingKey::ToggleCaseOperator(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::ToggleCase)
        }
        EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::ToggleCase,
        ),
        EditorVimPendingKey::YankLine(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::Yank)
        }
        EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::YankIntoRegister(register),
        ),
        EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        } => (operator_count, motion_count, EditorVimOperatorGoKind::Yank),
        EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::YankIntoRegister(register),
        ),
        _ => return None,
    };

    Some(EditorVimPendingKey::OperatorGoMotion {
        operator_count,
        motion_count,
        operator,
    })
}
