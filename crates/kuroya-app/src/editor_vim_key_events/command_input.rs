use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;
use std::cell::RefCell;

use super::input_edit::{
    EditorVimInputEdit, vim_clear_input, vim_delete_input_word_backward, vim_input_control_edit,
    vim_pop_input, vim_push_input, vim_take_input,
};
use super::search::{vim_search_input_accept_key, vim_search_input_cancel_key};
use super::{EditorVimPendingKey, VimKeyResult, vim_printable_key_char};

const VIM_COMMAND_SUBSTITUTE_MAX_MATCHES: usize = 10_000;

thread_local! {
    pub(super) static VIM_COMMAND_INPUT: RefCell<String> = const { RefCell::new(String::new()) };
}

pub(super) fn handle_vim_command_input_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    suppress_text: Option<char>,
) -> VimKeyResult {
    if vim_command_input_accept_key(key, modifiers) {
        *pending = None;
        return if vim_finish_pending_command(buffer) {
            VimKeyResult::changed(None)
        } else {
            VimKeyResult::handled(None)
        };
    }

    if let Some(edit) = vim_command_input_control_edit(key, modifiers) {
        match edit {
            EditorVimInputEdit::DeleteCharBackward => vim_pop_input(&VIM_COMMAND_INPUT),
            EditorVimInputEdit::Clear => vim_clear_command_input(),
            EditorVimInputEdit::DeleteWordBackward => {
                vim_delete_input_word_backward(&VIM_COMMAND_INPUT)
            }
        }
        *pending = Some(EditorVimPendingKey::CommandInput);
        return VimKeyResult::handled(None);
    }

    if let Some(ch) = vim_printable_key_char(key, modifiers) {
        vim_push_input(&VIM_COMMAND_INPUT, ch);
        *pending = Some(EditorVimPendingKey::CommandInput);
        return VimKeyResult::handled(suppress_text);
    }

    *pending = Some(EditorVimPendingKey::CommandInput);
    VimKeyResult::handled(None)
}

pub(super) fn vim_command_input_control_edit(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimInputEdit> {
    vim_input_control_edit(key, modifiers)
}

pub(super) fn vim_command_input_accept_key(key: Key, modifiers: Modifiers) -> bool {
    vim_search_input_accept_key(key, modifiers)
}

pub(super) fn vim_command_input_cancel_key(key: Key, modifiers: Modifiers) -> bool {
    vim_search_input_cancel_key(key, modifiers)
}

pub(super) fn vim_clear_command_input() {
    vim_clear_input(&VIM_COMMAND_INPUT);
}

fn vim_finish_pending_command(buffer: &mut TextBuffer) -> bool {
    let command = vim_take_input(&VIM_COMMAND_INPUT);
    let Some(substitute) = vim_parse_percent_substitute_command(&command) else {
        return false;
    };

    buffer.replace_all_matches(
        &substitute.query,
        &substitute.replacement,
        true,
        false,
        VIM_COMMAND_SUBSTITUTE_MAX_MATCHES,
    ) > 0
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorVimSubstituteCommand {
    query: String,
    replacement: String,
}

fn vim_parse_percent_substitute_command(command: &str) -> Option<EditorVimSubstituteCommand> {
    let rest = command.strip_prefix("%s")?;
    let mut chars = rest.chars().peekable();
    let delimiter = chars.next()?;
    if delimiter.is_ascii_alphanumeric() || delimiter.is_whitespace() || delimiter == '\\' {
        return None;
    }

    let query = vim_take_substitute_part(&mut chars, delimiter)?;
    let replacement = vim_take_substitute_part(&mut chars, delimiter)?;
    let flags = chars.collect::<String>();
    if query.is_empty() || !flags.contains('g') || flags.chars().any(|ch| ch != 'g') {
        return None;
    }

    Some(EditorVimSubstituteCommand { query, replacement })
}

fn vim_take_substitute_part(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    delimiter: char,
) -> Option<String> {
    let mut part = String::new();
    let mut escaped = false;
    for ch in chars.by_ref() {
        if escaped {
            if ch != delimiter && ch != '\\' {
                part.push('\\');
            }
            part.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
        } else if ch == delimiter {
            return Some(part);
        } else {
            part.push(ch);
        }
    }
    None
}
