mod change;
mod delete;

use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use self::change::handle_vim_change_pending_key_event;
use self::delete::handle_vim_delete_pending_key_event;
use super::super::*;

pub(super) fn handle_vim_change_delete_pending_key_event(
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
    handle_vim_change_pending_key_event(
        buffer,
        key,
        modifiers,
        mode,
        pending,
        last_char_find,
        unnamed_register,
        last_change,
        pending_key,
        suppress_text,
    )
    .or_else(|| {
        handle_vim_delete_pending_key_event(
            buffer,
            key,
            modifiers,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            pending_key,
            suppress_text,
        )
    })
}
