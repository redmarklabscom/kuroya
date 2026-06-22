use eframe::egui::{Key, Modifiers};

use super::super::EditorVimMode;
use super::modifiers::no_text_modifiers;
use super::movement::vim_line_column_motion_key;

pub(in crate::editor_vim_key_events) fn vim_normal_key_next_mode(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimMode> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    matches!(key, Key::I | Key::A | Key::O).then_some(EditorVimMode::Insert)
}

pub(in crate::editor_vim_key_events) fn vim_normal_key_is_handled(
    key: Key,
    modifiers: Modifiers,
) -> bool {
    if vim_escape_key(key, modifiers) {
        return true;
    }
    if modifiers.command || modifiers.alt {
        return false;
    }
    if modifiers.ctrl {
        return matches!(
            key,
            Key::R | Key::B | Key::D | Key::E | Key::F | Key::N | Key::P | Key::U | Key::Y
        ) && !modifiers.shift;
    }
    if vim_search_direction_for_key(key, modifiers).is_some() {
        return true;
    }
    if vim_line_column_motion_key(key, modifiers) {
        return true;
    }
    if matches!(key, Key::Comma | Key::Semicolon) && !modifiers.shift {
        return true;
    }
    if matches!(
        (key, modifiers.shift),
        (Key::Backtick, true)
            | (Key::CloseBracket, true)
            | (Key::Comma, true)
            | (Key::Equals, true)
            | (Key::Num3, true)
            | (Key::Num8, true)
            | (Key::OpenBracket, true)
            | (Key::Period, true)
            | (Key::Quote, true)
    ) {
        return true;
    }
    if key == Key::Period && !modifiers.shift {
        return true;
    }
    if matches!(
        (key, modifiers.shift),
        (Key::Backtick, false) | (Key::M, false) | (Key::Quote, false)
    ) {
        return true;
    }
    if matches!(
        key,
        Key::Backspace | Key::Enter | Key::Home | Key::End | Key::Space
    ) {
        return no_text_modifiers(modifiers);
    }
    matches!(
        key,
        Key::Escape
            | Key::H
            | Key::J
            | Key::K
            | Key::L
            | Key::Minus
            | Key::W
            | Key::E
            | Key::B
            | Key::C
            | Key::D
            | Key::Num0
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::N
            | Key::I
            | Key::A
            | Key::O
            | Key::P
            | Key::S
            | Key::X
            | Key::U
            | Key::Y
            | Key::G
    )
}

pub(in crate::editor_vim_key_events) fn vim_search_direction_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<bool> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::Slash, false) => Some(true),
        (Key::Slash, true) | (Key::Questionmark, _) => Some(false),
        _ => None,
    }
}

pub(in crate::editor_vim_key_events) fn insert_mode_key_can_mutate(
    key: Key,
    modifiers: Modifiers,
) -> bool {
    if ((modifiers.ctrl || modifiers.alt) && matches!(key, Key::Backspace | Key::Delete))
        || vim_insert_delete_char_backward_key(key, modifiers)
        || vim_insert_delete_line_backward_key(key, modifiers)
        || vim_insert_delete_word_backward_key(key, modifiers)
    {
        true
    } else if modifiers.command || modifiers.ctrl {
        matches!(key, Key::Z | Key::Y)
    } else {
        matches!(key, Key::Backspace | Key::Delete | Key::Enter | Key::Tab)
    }
}

pub(in crate::editor_vim_key_events) fn vim_escape_key(key: Key, modifiers: Modifiers) -> bool {
    (key == Key::Escape && no_text_modifiers(modifiers))
        || (key == Key::OpenBracket
            && modifiers.ctrl
            && !modifiers.shift
            && !modifiers.alt
            && !modifiers.command)
}

pub(in crate::editor_vim_key_events) fn vim_insert_delete_line_backward_key(
    key: Key,
    modifiers: Modifiers,
) -> bool {
    key == Key::U && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}

pub(in crate::editor_vim_key_events) fn vim_insert_delete_word_backward_key(
    key: Key,
    modifiers: Modifiers,
) -> bool {
    key == Key::W && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}

pub(in crate::editor_vim_key_events) fn vim_insert_delete_char_backward_key(
    key: Key,
    modifiers: Modifiers,
) -> bool {
    key == Key::H && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}
