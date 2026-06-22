mod char_find;
mod motion;
mod text_object;

use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use self::char_find::{
    handle_vim_change_char_find_into_register_key_event, handle_vim_change_char_find_key_event,
};
use self::motion::{
    handle_vim_change_line_into_register_motion_key_event, handle_vim_change_line_motion_key_event,
    handle_vim_change_motion_count_into_register_key_event,
    handle_vim_change_motion_count_key_event,
};
use self::text_object::{
    handle_vim_change_text_object_into_register_key_event, handle_vim_change_text_object_key_event,
};
use super::super::super::*;

pub(super) fn handle_vim_change_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    match (pending_key, key) {
        (EditorVimPendingKey::ChangeLine(count), Key::C) if !modifiers.shift => {
            let changed = vim_change_lines_into_register(buffer, count, unnamed_register);
            *mode = EditorVimMode::Insert;
            Some(vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::ChangeLines,
                count,
                suppress_text,
            ))
        }
        (
            EditorVimPendingKey::ChangeLineIntoRegister {
                operator_count,
                register,
            },
            Key::C,
        ) if !modifiers.shift => {
            let changed = vim_change_lines_into_named_register(
                buffer,
                operator_count,
                unnamed_register,
                register,
            );
            *mode = EditorVimMode::Insert;
            Some(vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::ChangeLinesIntoRegister(register),
                operator_count,
                suppress_text,
            ))
        }
        (EditorVimPendingKey::ChangeLine(operator_count), key) => {
            handle_vim_change_line_motion_key_event(
                buffer,
                key,
                modifiers,
                mode,
                pending,
                unnamed_register,
                last_change,
                operator_count,
                suppress_text,
            )
        }
        (
            EditorVimPendingKey::ChangeLineIntoRegister {
                operator_count,
                register,
            },
            key,
        ) => handle_vim_change_line_into_register_motion_key_event(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            unnamed_register,
            last_change,
            operator_count,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::ChangeMotionCount {
                operator_count,
                motion_count,
            },
            key,
        ) => handle_vim_change_motion_count_key_event(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            suppress_text,
        ),
        (
            EditorVimPendingKey::ChangeMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            },
            key,
        ) => handle_vim_change_motion_count_into_register_key_event(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::ChangeTextObject {
                operator_count,
                motion_count,
                scope,
            },
            key,
        ) => handle_vim_change_text_object_key_event(
            buffer,
            key,
            modifiers,
            mode,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            scope,
            suppress_text,
        ),
        (
            EditorVimPendingKey::ChangeTextObjectIntoRegister {
                operator_count,
                motion_count,
                scope,
                register,
            },
            key,
        ) => handle_vim_change_text_object_into_register_key_event(
            buffer,
            key,
            modifiers,
            mode,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            scope,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::ChangeCharFind {
                operator_count,
                motion_count,
                motion,
            },
            _,
        ) => handle_vim_change_char_find_key_event(
            buffer,
            key,
            modifiers,
            mode,
            last_char_find,
            unnamed_register,
            last_change,
            operator_count,
            motion_count,
            motion,
            suppress_text,
        ),
        (
            EditorVimPendingKey::ChangeCharFindIntoRegister {
                operator_count,
                motion_count,
                motion,
                register,
            },
            _,
        ) => handle_vim_change_char_find_into_register_key_event(
            buffer,
            key,
            modifiers,
            mode,
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
