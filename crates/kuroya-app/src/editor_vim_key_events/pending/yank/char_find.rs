use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::*;

pub(super) fn handle_vim_yank_char_find_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    match pending_key {
        EditorVimPendingKey::YankCharFind {
            operator_count,
            motion_count,
            motion,
        } => {
            let target = vim_printable_key_char(key, modifiers)?;
            let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
            *last_char_find = Some(EditorVimCharFind { motion, target });
            vim_yank_operator_motion(
                buffer,
                operator_count,
                motion_count,
                operator_motion,
                unnamed_register,
            );
            Some(VimKeyResult::handled(suppress_text))
        }
        EditorVimPendingKey::YankCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        } => {
            let target = vim_printable_key_char(key, modifiers)?;
            let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
            *last_char_find = Some(EditorVimCharFind { motion, target });
            vim_yank_operator_motion_into_named_register(
                buffer,
                operator_count,
                motion_count,
                operator_motion,
                unnamed_register,
                register,
            );
            Some(VimKeyResult::handled(suppress_text))
        }
        _ => None,
    }
}
