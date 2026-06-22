use super::super::super::EditorVimPendingKey;
use super::super::labels::{
    push_char_find_operator_prefix, push_count_prefix, push_counted_operator,
    push_text_object_prefix, vim_case_operator_label,
};

pub(super) fn push_case_pending_label(label: &mut String, pending: EditorVimPendingKey) -> bool {
    match pending {
        EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        } => {
            push_count_prefix(label, operator_count);
            label.push_str(vim_case_operator_label(conversion));
        }
        EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        } => {
            push_count_prefix(label, operator_count);
            label.push_str(vim_case_operator_label(conversion));
            push_count_prefix(label, motion_count);
        }
        EditorVimPendingKey::ToggleCaseOperator(count) => push_counted_operator(label, count, "g~"),
        EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        } => {
            push_count_prefix(label, operator_count);
            label.push_str("g~");
            push_count_prefix(label, motion_count);
        }
        EditorVimPendingKey::ConvertCaseCharFind {
            operator_count,
            motion_count,
            conversion,
            ..
        } => push_char_find_operator_prefix(
            label,
            operator_count,
            vim_case_operator_label(conversion),
            motion_count,
        ),
        EditorVimPendingKey::ToggleCaseCharFind {
            operator_count,
            motion_count,
            ..
        } => push_char_find_operator_prefix(label, operator_count, "g~", motion_count),
        EditorVimPendingKey::ConvertCaseTextObject {
            operator_count,
            motion_count,
            scope,
            conversion,
        } => push_text_object_prefix(
            label,
            operator_count,
            vim_case_operator_label(conversion),
            motion_count,
            scope,
        ),
        EditorVimPendingKey::ToggleCaseTextObject {
            operator_count,
            motion_count,
            scope,
        } => push_text_object_prefix(label, operator_count, "g~", motion_count, scope),
        _ => return false,
    }
    true
}
