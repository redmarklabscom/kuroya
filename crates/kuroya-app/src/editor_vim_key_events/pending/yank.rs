use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

mod char_find;
mod motion;
mod text_object;

use self::char_find::handle_vim_yank_char_find_pending_key_event;
use self::motion::handle_vim_yank_motion_pending_key_event;
use self::text_object::handle_vim_yank_text_object_pending_key_event;
use super::super::{EditorVimCharFind, EditorVimPendingKey, EditorVimRegister, VimKeyResult};

pub(super) fn handle_vim_yank_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if let Some(result) = handle_vim_yank_motion_pending_key_event(
        buffer,
        key,
        modifiers,
        pending,
        unnamed_register,
        pending_key,
        suppress_text,
    ) {
        return Some(result);
    }
    if let Some(result) = handle_vim_yank_text_object_pending_key_event(
        buffer,
        key,
        modifiers,
        unnamed_register,
        pending_key,
        suppress_text,
    ) {
        return Some(result);
    }
    handle_vim_yank_char_find_pending_key_event(
        buffer,
        key,
        modifiers,
        last_char_find,
        unnamed_register,
        pending_key,
        suppress_text,
    )
}
