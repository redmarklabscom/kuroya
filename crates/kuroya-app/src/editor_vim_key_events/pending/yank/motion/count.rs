use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::super::*;
use super::super::super::operator_motion::{
    VimPendingOperator, handle_vim_pending_operator_motion_transition,
};

pub(super) fn handle_yank_motion_count_key(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    unnamed_register: &mut Option<EditorVimRegister>,
    operator_count: usize,
    motion_count: usize,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if let Some(result) = handle_vim_pending_operator_motion_transition(
        key,
        modifiers,
        pending,
        VimPendingOperator::Yank,
        None,
        operator_count,
        motion_count,
        true,
        suppress_text,
    ) {
        return Some(result);
    }
    if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
        vim_yank_operator_motion(
            buffer,
            operator_count,
            motion_count,
            motion,
            unnamed_register,
        );
        return Some(VimKeyResult::handled(suppress_text));
    }
    None
}

pub(super) fn handle_yank_motion_count_into_register_key(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    unnamed_register: &mut Option<EditorVimRegister>,
    operator_count: usize,
    motion_count: usize,
    register: EditorVimNamedRegister,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if let Some(result) = handle_vim_pending_operator_motion_transition(
        key,
        modifiers,
        pending,
        VimPendingOperator::Yank,
        Some(register),
        operator_count,
        motion_count,
        true,
        suppress_text,
    ) {
        return Some(result);
    }
    if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
        vim_yank_operator_motion_into_named_register(
            buffer,
            operator_count,
            motion_count,
            motion,
            unnamed_register,
            register,
        );
        return Some(VimKeyResult::handled(suppress_text));
    }
    None
}
