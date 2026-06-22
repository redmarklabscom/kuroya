use eframe::egui::{Key, Modifiers};

use super::super::{
    EditorVimCharFindMotion, EditorVimOperatorMotion, no_text_modifiers, vim_line_column_motion_key,
};

pub(in crate::editor_vim_key_events) fn vim_operator_motion_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimOperatorMotion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if vim_line_column_motion_key(key, modifiers) {
        return Some(EditorVimOperatorMotion::LineColumn);
    }
    if key == Key::Home && no_text_modifiers(modifiers) {
        return Some(EditorVimOperatorMotion::LineColumnStart);
    }
    if key == Key::End && no_text_modifiers(modifiers) {
        return Some(EditorVimOperatorMotion::LineEnd);
    }
    match (key, modifiers.shift) {
        (Key::B, false) => Some(EditorVimOperatorMotion::WordBackward),
        (Key::B, true) => Some(EditorVimOperatorMotion::BigWordBackward),
        (Key::E, false) => Some(EditorVimOperatorMotion::WordEnd),
        (Key::E, true) => Some(EditorVimOperatorMotion::BigWordEnd),
        (Key::Backspace, false) => Some(EditorVimOperatorMotion::CharacterBackward),
        (Key::H, false) => Some(EditorVimOperatorMotion::CharacterBackward),
        (Key::L, false) => Some(EditorVimOperatorMotion::CharacterForward),
        (Key::Space, false) => Some(EditorVimOperatorMotion::CharacterForward),
        (Key::W, false) => Some(EditorVimOperatorMotion::WordForward),
        (Key::W, true) => Some(EditorVimOperatorMotion::BigWordForward),
        (Key::Num0, false) => Some(EditorVimOperatorMotion::LineColumnStart),
        (Key::Num4, true) => Some(EditorVimOperatorMotion::LineEnd),
        (Key::Num5, true) => Some(EditorVimOperatorMotion::MatchingBracket),
        (Key::Num6, true) => Some(EditorVimOperatorMotion::LineFirstNonWhitespace),
        (Key::CloseBracket, true) => Some(EditorVimOperatorMotion::ParagraphForward),
        (Key::OpenBracket, true) => Some(EditorVimOperatorMotion::ParagraphBackward),
        (Key::Num3, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: false,
            whole_word: true,
        }),
        (Key::Num8, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: true,
            whole_word: true,
        }),
        (Key::N, false) => Some(EditorVimOperatorMotion::SearchRepeat { reverse: false }),
        (Key::N, true) => Some(EditorVimOperatorMotion::SearchRepeat { reverse: true }),
        _ => None,
    }
}

pub(in crate::editor_vim_key_events) fn vim_operator_go_motion_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimOperatorMotion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::E, false) => Some(EditorVimOperatorMotion::WordEndBackward),
        (Key::E, true) => Some(EditorVimOperatorMotion::BigWordEndBackward),
        (Key::N, false) => Some(EditorVimOperatorMotion::SearchMatch { reverse: false }),
        (Key::N, true) => Some(EditorVimOperatorMotion::SearchMatch { reverse: true }),
        (Key::Num3, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: false,
            whole_word: false,
        }),
        (Key::Num8, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: true,
            whole_word: false,
        }),
        _ => None,
    }
}

pub(in crate::editor_vim_key_events) fn vim_operator_char_find_motion_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimCharFindMotion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::F, false) => Some(EditorVimCharFindMotion::FindForward),
        (Key::F, true) => Some(EditorVimCharFindMotion::FindBackward),
        (Key::T, false) => Some(EditorVimCharFindMotion::TillForward),
        (Key::T, true) => Some(EditorVimCharFindMotion::TillBackward),
        _ => None,
    }
}
