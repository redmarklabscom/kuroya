use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::super::*;

pub(super) fn handle_vim_delete_text_object_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    let kind = vim_text_object_kind_for_key(key, modifiers)?;
    let count = vim_combined_count(operator_count, motion_count);
    Some(vim_repeatable_change_result(
        vim_apply_text_object(
            buffer,
            operator_count,
            motion_count,
            scope,
            kind,
            unnamed_register,
        ),
        last_change,
        EditorVimRepeatAction::DeleteTextObject { scope, kind },
        count,
        suppress_text,
    ))
}

pub(super) fn handle_vim_delete_text_object_into_register_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    register: EditorVimNamedRegister,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    let kind = vim_text_object_kind_for_key(key, modifiers)?;
    let count = vim_combined_count(operator_count, motion_count);
    Some(vim_repeatable_change_result(
        vim_apply_text_object_into_named_register(
            buffer,
            operator_count,
            motion_count,
            scope,
            kind,
            unnamed_register,
            register,
        ),
        last_change,
        EditorVimRepeatAction::DeleteTextObjectIntoRegister {
            scope,
            kind,
            register,
        },
        count,
        suppress_text,
    ))
}
