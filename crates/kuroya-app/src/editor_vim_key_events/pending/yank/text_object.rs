use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::*;

pub(super) fn handle_vim_yank_text_object_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    unnamed_register: &mut Option<EditorVimRegister>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    match pending_key {
        EditorVimPendingKey::YankTextObject {
            operator_count,
            motion_count,
            scope,
        } => {
            let kind = vim_text_object_kind_for_key(key, modifiers)?;
            vim_yank_text_object(
                buffer,
                operator_count,
                motion_count,
                scope,
                kind,
                unnamed_register,
            );
            Some(VimKeyResult::handled(suppress_text))
        }
        EditorVimPendingKey::YankTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        } => {
            let kind = vim_text_object_kind_for_key(key, modifiers)?;
            vim_yank_text_object_into_named_register(
                buffer,
                operator_count,
                motion_count,
                scope,
                kind,
                unnamed_register,
                register,
            );
            Some(VimKeyResult::handled(suppress_text))
        }
        _ => None,
    }
}
