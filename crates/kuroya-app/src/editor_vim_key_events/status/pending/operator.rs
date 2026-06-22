use super::super::super::EditorVimPendingKey;
use super::super::labels::{push_counted_operator, push_register_prefix, push_text_object_prefix};

pub(super) fn push_operator_pending_label(
    label: &mut String,
    pending: EditorVimPendingKey,
) -> bool {
    match pending {
        EditorVimPendingKey::ChangeLine(count)
        | EditorVimPendingKey::ChangeMotionCount {
            operator_count: count,
            ..
        }
        | EditorVimPendingKey::ChangeCharFind {
            operator_count: count,
            ..
        } => push_counted_operator(label, count, "c"),
        EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }
        | EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            register,
            ..
        }
        | EditorVimPendingKey::ChangeCharFindIntoRegister {
            operator_count,
            register,
            ..
        } => {
            push_register_prefix(label, register);
            push_counted_operator(label, operator_count, "c");
        }
        EditorVimPendingKey::DeleteLine(count)
        | EditorVimPendingKey::DeleteMotionCount {
            operator_count: count,
            ..
        }
        | EditorVimPendingKey::DeleteCharFind {
            operator_count: count,
            ..
        } => push_counted_operator(label, count, "d"),
        EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }
        | EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            register,
            ..
        }
        | EditorVimPendingKey::DeleteCharFindIntoRegister {
            operator_count,
            register,
            ..
        } => {
            push_register_prefix(label, register);
            push_counted_operator(label, operator_count, "d");
        }
        EditorVimPendingKey::YankLine(count)
        | EditorVimPendingKey::YankMotionCount {
            operator_count: count,
            ..
        }
        | EditorVimPendingKey::YankCharFind {
            operator_count: count,
            ..
        } => push_counted_operator(label, count, "y"),
        EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }
        | EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            register,
            ..
        }
        | EditorVimPendingKey::YankCharFindIntoRegister {
            operator_count,
            register,
            ..
        } => {
            push_register_prefix(label, register);
            push_counted_operator(label, operator_count, "y");
        }
        EditorVimPendingKey::ChangeTextObject {
            operator_count,
            motion_count,
            scope,
        } => push_text_object_prefix(label, operator_count, "c", motion_count, scope),
        EditorVimPendingKey::ChangeTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        } => {
            push_register_prefix(label, register);
            push_text_object_prefix(label, operator_count, "c", motion_count, scope);
        }
        EditorVimPendingKey::DeleteTextObject {
            operator_count,
            motion_count,
            scope,
        } => push_text_object_prefix(label, operator_count, "d", motion_count, scope),
        EditorVimPendingKey::DeleteTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        } => {
            push_register_prefix(label, register);
            push_text_object_prefix(label, operator_count, "d", motion_count, scope);
        }
        EditorVimPendingKey::YankTextObject {
            operator_count,
            motion_count,
            scope,
        } => push_text_object_prefix(label, operator_count, "y", motion_count, scope),
        EditorVimPendingKey::YankTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        } => {
            push_register_prefix(label, register);
            push_text_object_prefix(label, operator_count, "y", motion_count, scope);
        }
        _ => return false,
    }
    true
}
