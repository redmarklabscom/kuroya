use super::{
    EditorVimCharFind, EditorVimCharFindMotion, EditorVimMode, EditorVimNamedRegister,
    EditorVimPendingKey, EditorVimRegister, EditorVimRegisterKind, VIM_DEFAULT_CTRL_SCROLL_LINES,
    VIM_DEFAULT_PAGE_SCROLL_LINES, VIM_SEARCH_INPUT, VIM_SEARCHES, handle_vim_editor_key_event,
    handle_vim_editor_key_event_with_repeat_state, handle_vim_editor_key_event_with_state,
    handle_vim_editor_key_event_with_state_and_indent, vim_apply_char_find,
    vim_clear_named_registers, vim_events_include_mutation, vim_named_register,
    vim_open_line_above_text, vim_open_line_below_text, vim_pending_search_status_label,
    vim_record_insert_replay_key_with_auto_indent, vim_record_inserted_text,
    vim_search_word_target, vim_set_last_search, vim_text_after_suppression,
};
use eframe::egui::{Event, Key, Modifiers};
use kuroya_core::TextBuffer;
use std::collections::VecDeque;

mod case;
mod char_find;
mod edits;
mod insert;
mod motions;
mod operators;
mod registers;
mod scroll;
mod search;
mod text_objects;
mod visual;

fn key_event(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
