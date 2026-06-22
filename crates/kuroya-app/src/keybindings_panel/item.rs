use crate::{
    keybinding_parse::normalize_key_chord,
    keybindings::{keybinding_items, keybinding_search_text},
    path_display::sanitized_display_label_cow,
};
use kuroya_core::{Command, keymap::KeyBinding};
use std::borrow::Cow;

use super::KEYBINDING_TEXT_MAX_CHARS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::keybindings_panel) struct KeybindingPanelItem {
    pub(in crate::keybindings_panel) chord: String,
    pub(in crate::keybindings_panel) command: Command,
    pub(in crate::keybindings_panel) label: String,
    pub(in crate::keybindings_panel) search_text: String,
}

pub(in crate::keybindings_panel) fn sanitized_keybinding_items(
    bindings: &[KeyBinding],
) -> Vec<KeybindingPanelItem> {
    keybinding_items(bindings)
        .into_iter()
        .map(sanitized_keybinding_item)
        .collect()
}

pub(in crate::keybindings_panel) fn sanitized_keybinding_item(
    (mut chord, command, mut label): (String, Command, String),
) -> KeybindingPanelItem {
    sanitize_keybinding_chord_in_place(&mut chord);
    sanitize_keybinding_label_in_place(&mut label);
    let search_text = keybinding_search_text(chord.as_str(), label.as_str());
    KeybindingPanelItem {
        chord,
        command,
        label,
        search_text,
    }
}

pub(in crate::keybindings_panel) fn sanitize_keybinding_chord_in_place(chord: &mut String) {
    let Cow::Owned(sanitized) = sanitize_keybinding_chord_cow(chord.as_str()) else {
        return;
    };
    if sanitized != chord.as_str() {
        *chord = sanitized;
    }
}

pub(in crate::keybindings_panel) fn sanitize_keybinding_label_in_place(label: &mut String) {
    let Cow::Owned(sanitized) = sanitize_keybinding_label_cow(label.as_str()) else {
        return;
    };
    if sanitized != label.as_str() {
        *label = sanitized;
    }
}

#[cfg(test)]
pub(in crate::keybindings_panel) fn sanitize_keybinding_chord(chord: &str) -> String {
    sanitize_keybinding_chord_cow(chord).into_owned()
}

pub(in crate::keybindings_panel) fn sanitize_keybinding_chord_cow(chord: &str) -> Cow<'_, str> {
    if chord.is_empty() {
        return Cow::Borrowed("");
    }
    if keybinding_chord_is_display_normalized(chord) {
        return Cow::Borrowed(chord);
    }
    if let Some(normalized) = normalize_key_chord(chord) {
        return if normalized == chord {
            Cow::Borrowed(chord)
        } else {
            Cow::Owned(normalized)
        };
    }
    sanitized_display_label_cow(chord, KEYBINDING_TEXT_MAX_CHARS, "Invalid shortcut")
}

fn keybinding_chord_is_display_normalized(chord: &str) -> bool {
    let mut next_modifier_index = 0usize;
    let mut saw_key = false;

    for part in chord.split('+') {
        if part.is_empty() || part.chars().any(char::is_whitespace) {
            return false;
        }

        if let Some(modifier_index) = keybinding_display_modifier_index(part) {
            if saw_key || modifier_index < next_modifier_index {
                return false;
            }
            next_modifier_index = modifier_index + 1;
        } else {
            if saw_key || !keybinding_display_key_name_is_canonical(part) {
                return false;
            }
            saw_key = true;
        }
    }

    saw_key
}

fn keybinding_display_modifier_index(part: &str) -> Option<usize> {
    match part {
        "Ctrl" => Some(0),
        "Alt" => Some(1),
        "Shift" => Some(2),
        "Cmd" => Some(3),
        _ => None,
    }
}

fn keybinding_display_key_name_is_canonical(part: &str) -> bool {
    matches!(
        part,
        "A" | "B"
            | "C"
            | "D"
            | "E"
            | "F"
            | "G"
            | "H"
            | "I"
            | "J"
            | "K"
            | "L"
            | "M"
            | "N"
            | "O"
            | "P"
            | "Q"
            | "R"
            | "S"
            | "T"
            | "U"
            | "V"
            | "W"
            | "X"
            | "Y"
            | "Z"
            | "0"
            | "1"
            | "2"
            | "3"
            | "4"
            | "5"
            | "6"
            | "7"
            | "8"
            | "9"
            | "Up"
            | "Down"
            | "Left"
            | "Right"
            | "Enter"
            | "Tab"
            | "Space"
            | "Backspace"
            | "Delete"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F8"
            | "F9"
            | "F10"
            | "F11"
            | "F12"
            | "`"
            | "\\"
            | "["
            | "]"
            | ","
            | "."
            | "/"
            | "-"
            | "="
            | ";"
            | "'"
    )
}

#[cfg(test)]
pub(in crate::keybindings_panel) fn sanitize_keybinding_label(label: &str) -> String {
    sanitize_keybinding_label_cow(label).into_owned()
}

pub(in crate::keybindings_panel) fn sanitize_keybinding_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, KEYBINDING_TEXT_MAX_CHARS, "Unnamed command")
}
