use eframe::egui::{Key, Modifiers};

use super::super::super::super::{
    EditorVimOperatorGoKind, EditorVimPendingKey, vim_case_conversion_repeated_operator_key,
};
use super::super::super::motions::{vim_operator_go_motion_for_key, vim_operator_motion_for_key};
use super::super::super::text_objects::vim_text_object_kind_for_key;

pub(super) fn vim_operator_pending_key_can_mutate(
    key: Key,
    modifiers: Modifiers,
    pending: Option<EditorVimPendingKey>,
) -> Option<bool> {
    if let Some(EditorVimPendingKey::OperatorGoMotion { operator, .. }) = pending
        && vim_operator_go_motion_for_key(key, modifiers).is_some()
    {
        return Some(vim_operator_go_kind_can_mutate(operator));
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::ChangeLine(_)), Key::C, false)
            | (
                Some(EditorVimPendingKey::ChangeLineIntoRegister { .. }),
                Key::C,
                false
            )
            | (Some(EditorVimPendingKey::DeleteLine(_)), Key::D, false)
            | (
                Some(EditorVimPendingKey::DeleteLineIntoRegister { .. }),
                Key::D,
                false
            )
            | (Some(EditorVimPendingKey::Go(_)), Key::J, true)
    ) {
        return Some(true);
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::YankLine(_)), Key::Y, false)
            | (
                Some(EditorVimPendingKey::YankLineIntoRegister { .. }),
                Key::Y,
                false
            )
    ) {
        return Some(false);
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::IndentLine(_)), Key::Period, true)
            | (Some(EditorVimPendingKey::OutdentLine(_)), Key::Comma, true)
    ) {
        return Some(true);
    }
    if let Some(
        EditorVimPendingKey::ConvertCaseOperator { conversion, .. }
        | EditorVimPendingKey::ConvertCaseMotionCount { conversion, .. },
    ) = pending
        && vim_case_conversion_repeated_operator_key(conversion, key, modifiers)
    {
        return Some(true);
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ConvertCaseOperator { .. }
                | EditorVimPendingKey::ConvertCaseMotionCount { .. }
        )
    ) && key == Key::U
    {
        return Some(false);
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeTextObject { .. }
                | EditorVimPendingKey::ChangeTextObjectIntoRegister { .. }
                | EditorVimPendingKey::DeleteTextObject { .. }
                | EditorVimPendingKey::DeleteTextObjectIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseTextObject { .. }
                | EditorVimPendingKey::ToggleCaseTextObject { .. }
        )
    ) && vim_text_object_kind_for_key(key, modifiers).is_some()
    {
        return Some(true);
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeLine(_)
                | EditorVimPendingKey::ChangeLineIntoRegister { .. }
                | EditorVimPendingKey::ChangeMotionCount { .. }
                | EditorVimPendingKey::ChangeMotionCountIntoRegister { .. }
                | EditorVimPendingKey::DeleteLine(_)
                | EditorVimPendingKey::DeleteMotionCount { .. }
                | EditorVimPendingKey::DeleteLineIntoRegister { .. }
                | EditorVimPendingKey::DeleteMotionCountIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseOperator { .. }
                | EditorVimPendingKey::ConvertCaseMotionCount { .. }
                | EditorVimPendingKey::ToggleCaseOperator(_)
                | EditorVimPendingKey::ToggleCaseMotionCount { .. }
        )
    ) && vim_operator_motion_for_key(key, modifiers).is_some()
    {
        return Some(true);
    }
    None
}

fn vim_operator_go_kind_can_mutate(operator: EditorVimOperatorGoKind) -> bool {
    !matches!(
        operator,
        EditorVimOperatorGoKind::Yank | EditorVimOperatorGoKind::YankIntoRegister(_)
    )
}
