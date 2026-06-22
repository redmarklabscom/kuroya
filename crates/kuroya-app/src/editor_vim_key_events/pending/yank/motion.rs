use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

mod count;
mod line;

use self::count::{handle_yank_motion_count_into_register_key, handle_yank_motion_count_key};
use self::line::{handle_yank_line_into_register_motion_key, handle_yank_line_motion_key};
use super::super::super::{
    EditorVimPendingKey, EditorVimRegister, VimKeyResult, vim_yank_lines,
    vim_yank_lines_into_named_register,
};

pub(super) fn handle_vim_yank_motion_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    unnamed_register: &mut Option<EditorVimRegister>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    match (pending_key, key) {
        (EditorVimPendingKey::YankLine(count), Key::Y) if !modifiers.shift => {
            vim_yank_lines(buffer, count, unnamed_register);
            Some(VimKeyResult::handled(suppress_text))
        }
        (
            EditorVimPendingKey::YankLineIntoRegister {
                operator_count,
                register,
            },
            Key::Y,
        ) if !modifiers.shift => {
            vim_yank_lines_into_named_register(buffer, operator_count, unnamed_register, register);
            Some(VimKeyResult::handled(suppress_text))
        }
        (EditorVimPendingKey::YankLine(operator_count), key) => handle_yank_line_motion_key(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            operator_count,
            suppress_text,
        ),
        (
            EditorVimPendingKey::YankLineIntoRegister {
                operator_count,
                register,
            },
            key,
        ) => handle_yank_line_into_register_motion_key(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            operator_count,
            register,
            suppress_text,
        ),
        (
            EditorVimPendingKey::YankMotionCount {
                operator_count,
                motion_count,
            },
            key,
        ) => handle_yank_motion_count_key(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            operator_count,
            motion_count,
            suppress_text,
        ),
        (
            EditorVimPendingKey::YankMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            },
            key,
        ) => handle_yank_motion_count_into_register_key(
            buffer,
            key,
            modifiers,
            pending,
            unnamed_register,
            operator_count,
            motion_count,
            register,
            suppress_text,
        ),
        _ => None,
    }
}
