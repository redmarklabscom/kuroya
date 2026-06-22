mod char_find;
mod motion;
mod text_object;

use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use self::char_find::{
    handle_vim_delete_char_find_into_register_key_event, handle_vim_delete_char_find_key_event,
};
use self::motion::{
    handle_vim_delete_line_into_register_motion_key_event, handle_vim_delete_line_motion_key_event,
    handle_vim_delete_motion_count_into_register_key_event,
    handle_vim_delete_motion_count_key_event,
};
use self::text_object::{
    handle_vim_delete_text_object_into_register_key_event, handle_vim_delete_text_object_key_event,
};
use super::super::super::*;

pub(super) fn handle_vim_delete_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    match (pending_key, key) {
        (EditorVimPendingKey::DeleteLine(count), Key::D) if !modifiers.shift => {
            Some(vim_repeatable_change_result(
                vim_delete_lines_into_register(buffer, count, unnamed_register),
                last_change,
                EditorVimRepeatAction::DeleteLines,
                count,
                suppress_text,
            ))
        }
        (
            EditorVimPendingKey::DeleteLineIntoRegister {
                operator_count,
                register,
            },
            Key::D,
        ) if !modifiers.shift => Some(vim_repeatable_change_result(
            vim_delete_lines_into_named_register(
                buffer,
                operator_count,
                unnamed_register,
                register,
            ),
            last_change,
            EditorVimRepeatAction::DeleteLinesIntoRegister(register),
            operator_count,
            suppress_text,
        )),
        (EditorVimPendingKey::DeleteLine(operator_count), key) => {
            handle_vim_delete_line_motion_key_event(
                buffer,
                key,
                modifiers,
                pending,
                unnamed_register,
                last_change,
                operator_count,
                suppress_text,
            )
        }
        (
            EditorVimPendingKey::DeleteLineIntoRegister {
                operator_count,
                register,
            },
            key,
        ) => handle_vim_delete_line_into_register_motion_key_event(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            last_change,
            operator_count,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::DeleteMotionCount {
                operator_count,
                motion_count,
            },
            key,
        ) => handle_vim_delete_motion_count_key_event(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            suppress_text,
        ),
        (
            EditorVimPendingKey::DeleteMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            },
            key,
        ) => handle_vim_delete_motion_count_into_register_key_event(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::DeleteTextObject {
                operator_count,
                motion_count,
                scope,
            },
            key,
        ) => handle_vim_delete_text_object_key_event(
            buffer,
            key,
            modifiers,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            scope,
            suppress_text,
        ),
        (
            EditorVimPendingKey::DeleteTextObjectIntoRegister {
                operator_count,
                motion_count,
                scope,
                register,
            },
            key,
        ) => handle_vim_delete_text_object_into_register_key_event(
            buffer,
            key,
            modifiers,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            scope,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::DeleteCharFind {
                operator_count,
                motion_count,
                motion,
            },
            _,
        ) => handle_vim_delete_char_find_key_event(
            buffer,
            key,
            modifiers,
            last_char_find,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            motion,
            suppress_text,
        ),
        (
            EditorVimPendingKey::DeleteCharFindIntoRegister {
                operator_count,
                motion_count,
                motion,
                register,
            },
            _,
        ) => handle_vim_delete_char_find_into_register_key_event(
            buffer,
            key,
            modifiers,
            last_char_find,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            motion,
            register,
            suppress_text,
        ),
        _ => None,
    }
}
