use crate::keybinding_chords::names::key_name_for_chord;
use eframe::egui::{Key, Modifiers};

mod names;

pub(crate) fn keybinding_chord_from_key(key: Key, modifiers: Modifiers) -> Option<String> {
    let key_name = keybinding_key_name(key)?;
    let mut chord = String::with_capacity(key_name.len() + 20);
    if modifiers.ctrl {
        push_keybinding_chord_part(&mut chord, "Ctrl");
    }
    if modifiers.alt {
        push_keybinding_chord_part(&mut chord, "Alt");
    }
    if modifiers.shift {
        push_keybinding_chord_part(&mut chord, "Shift");
    }
    if modifiers.mac_cmd {
        push_keybinding_chord_part(&mut chord, "Cmd");
    }
    push_keybinding_chord_part(&mut chord, key_name);
    Some(chord)
}

pub(crate) fn keybinding_key_name(key: Key) -> Option<&'static str> {
    key_name_for_chord(key)
}

fn push_keybinding_chord_part(chord: &mut String, part: &str) {
    if !chord.is_empty() {
        chord.push('+');
    }
    chord.push_str(part);
}

pub(crate) fn keybinding_requires_primary_modifier(key: Key) -> bool {
    !matches!(
        key,
        Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
            | Key::Escape
            | Key::F1
            | Key::F2
            | Key::F3
            | Key::F4
            | Key::F5
            | Key::F6
            | Key::F7
            | Key::F8
            | Key::F9
            | Key::F10
            | Key::F11
            | Key::F12
    )
}
