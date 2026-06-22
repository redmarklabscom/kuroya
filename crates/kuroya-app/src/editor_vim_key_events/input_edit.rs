use eframe::egui::{Key, Modifiers};
use std::{cell::RefCell, thread::LocalKey};

use super::{
    no_text_modifiers, vim_insert_delete_char_backward_key, vim_insert_delete_line_backward_key,
    vim_insert_delete_word_backward_key,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::editor_vim_key_events) enum EditorVimInputEdit {
    DeleteCharBackward,
    Clear,
    DeleteWordBackward,
}

pub(in crate::editor_vim_key_events) fn vim_input_control_edit(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimInputEdit> {
    if (key == Key::Backspace && no_text_modifiers(modifiers))
        || vim_insert_delete_char_backward_key(key, modifiers)
    {
        Some(EditorVimInputEdit::DeleteCharBackward)
    } else if vim_insert_delete_line_backward_key(key, modifiers) {
        Some(EditorVimInputEdit::Clear)
    } else if vim_insert_delete_word_backward_key(key, modifiers) {
        Some(EditorVimInputEdit::DeleteWordBackward)
    } else {
        None
    }
}

pub(in crate::editor_vim_key_events) fn vim_clear_input(input: &'static LocalKey<RefCell<String>>) {
    input.with(|input| input.borrow_mut().clear());
}

pub(in crate::editor_vim_key_events) fn vim_push_input(
    input: &'static LocalKey<RefCell<String>>,
    ch: char,
) {
    input.with(|input| input.borrow_mut().push(ch));
}

pub(in crate::editor_vim_key_events) fn vim_pop_input(input: &'static LocalKey<RefCell<String>>) {
    input.with(|input| {
        input.borrow_mut().pop();
    });
}

pub(in crate::editor_vim_key_events) fn vim_delete_input_word_backward(
    input: &'static LocalKey<RefCell<String>>,
) {
    input.with(|input| {
        let mut value = input.borrow_mut();
        while value
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace())
        {
            value.pop();
        }
        while value
            .chars()
            .next_back()
            .is_some_and(|ch| !ch.is_whitespace())
        {
            value.pop();
        }
    });
}

pub(in crate::editor_vim_key_events) fn vim_take_input(
    input: &'static LocalKey<RefCell<String>>,
) -> String {
    input.with(|input| std::mem::take(&mut *input.borrow_mut()))
}
