use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::super::*;

pub(super) fn handle_vim_delete_char_find_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimCharFindMotion,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    let target = vim_printable_key_char(key, modifiers)?;
    let count = vim_combined_count(operator_count, motion_count);
    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
    *last_char_find = Some(EditorVimCharFind { motion, target });
    Some(vim_repeatable_change_result(
        vim_apply_operator_motion(
            buffer,
            operator_count,
            motion_count,
            operator_motion,
            unnamed_register,
        ),
        last_change,
        EditorVimRepeatAction::DeleteOperatorMotion(operator_motion),
        count,
        suppress_text,
    ))
}

pub(super) fn handle_vim_delete_char_find_into_register_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimCharFindMotion,
    register: EditorVimNamedRegister,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    let target = vim_printable_key_char(key, modifiers)?;
    let count = vim_combined_count(operator_count, motion_count);
    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
    *last_char_find = Some(EditorVimCharFind { motion, target });
    Some(vim_repeatable_change_result(
        vim_apply_operator_motion_into_named_register(
            buffer,
            operator_count,
            motion_count,
            operator_motion,
            unnamed_register,
            register,
        ),
        last_change,
        EditorVimRepeatAction::DeleteOperatorMotionIntoRegister {
            motion: operator_motion,
            register,
        },
        count,
        suppress_text,
    ))
}
