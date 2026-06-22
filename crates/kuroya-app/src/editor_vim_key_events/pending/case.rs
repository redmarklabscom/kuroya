mod convert;
mod toggle;

use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use self::convert::handle_vim_convert_case_pending_key_event;
use self::toggle::handle_vim_toggle_case_pending_key_event;
use super::super::*;

pub(super) fn handle_vim_case_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    last_change: &mut Option<EditorVimLastChange>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if let Some(result) = handle_vim_convert_case_pending_key_event(
        buffer,
        key,
        modifiers,
        pending,
        last_char_find,
        last_change,
        pending_key,
        suppress_text,
    ) {
        return Some(result);
    }
    handle_vim_toggle_case_pending_key_event(
        buffer,
        key,
        modifiers,
        pending,
        last_char_find,
        last_change,
        pending_key,
        suppress_text,
    )
}
