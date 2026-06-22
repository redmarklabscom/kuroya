use eframe::egui::{Key, Modifiers};

use super::super::{
    EditorVimTextObjectKind, EditorVimTextObjectScope, no_text_modifiers, vim_printable_key_char,
};

pub(in crate::editor_vim_key_events) fn vim_text_object_scope_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimTextObjectScope> {
    if !no_text_modifiers(modifiers) {
        return None;
    }
    match key {
        Key::I => Some(EditorVimTextObjectScope::Inner),
        Key::A => Some(EditorVimTextObjectScope::Outer),
        _ => None,
    }
}

pub(in crate::editor_vim_key_events) fn vim_text_object_kind_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimTextObjectKind> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match vim_printable_key_char(key, modifiers)? {
        'W' => Some(EditorVimTextObjectKind::BigWord),
        'w' => Some(EditorVimTextObjectKind::Word),
        'p' => Some(EditorVimTextObjectKind::Paragraph),
        's' => Some(EditorVimTextObjectKind::Sentence),
        '(' | ')' => Some(EditorVimTextObjectKind::Block {
            open: '(',
            close: ')',
        }),
        '[' | ']' => Some(EditorVimTextObjectKind::Block {
            open: '[',
            close: ']',
        }),
        '<' | '>' => Some(EditorVimTextObjectKind::Block {
            open: '<',
            close: '>',
        }),
        '{' | '}' => Some(EditorVimTextObjectKind::Block {
            open: '{',
            close: '}',
        }),
        ch @ ('"' | '\'' | '`') => Some(EditorVimTextObjectKind::Quote { quote: ch }),
        _ => None,
    }
}
