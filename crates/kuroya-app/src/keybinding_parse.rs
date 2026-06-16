use crate::keybinding_chords::keybinding_key_name;
use eframe::egui::{Key, KeyboardShortcut, Modifiers};

const KEYBINDING_CHORD_MAX_CHARS: usize = 64;
const KEYBINDING_CHORD_MAX_PARTS: usize = 5;

pub(crate) fn parse_key_chord(chord: &str) -> Option<KeyboardShortcut> {
    let parsed = parse_key_chord_parts(chord)?;
    Some(KeyboardShortcut::new(parsed.modifiers, parsed.key))
}

pub(crate) fn normalize_key_chord(chord: &str) -> Option<String> {
    let parsed = parse_key_chord_parts(chord)?;
    let key_name = keybinding_key_name(parsed.key)?;
    let mut normalized = String::with_capacity(key_name.len() + 20);
    if parsed.has_ctrl {
        push_key_chord_part(&mut normalized, "Ctrl");
    }
    if parsed.has_alt {
        push_key_chord_part(&mut normalized, "Alt");
    }
    if parsed.has_shift {
        push_key_chord_part(&mut normalized, "Shift");
    }
    if parsed.has_command {
        push_key_chord_part(&mut normalized, "Cmd");
    }
    push_key_chord_part(&mut normalized, key_name);
    Some(normalized)
}

#[derive(Debug, Clone, Copy)]
struct ParsedKeyChord {
    modifiers: Modifiers,
    key: Key,
    has_ctrl: bool,
    has_alt: bool,
    has_shift: bool,
    has_command: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyChordModifier {
    Ctrl,
    Shift,
    Alt,
    Command,
}

fn push_key_chord_part(chord: &mut String, part: &str) {
    if !chord.is_empty() {
        chord.push('+');
    }
    chord.push_str(part);
}

fn parse_key_chord_parts(chord: &str) -> Option<ParsedKeyChord> {
    if key_chord_exceeds_max_chars(chord) {
        return None;
    }

    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    let mut has_ctrl = false;
    let mut has_shift = false;
    let mut has_alt = false;
    let mut has_command = false;
    let mut part_count = 0usize;

    for part in chord.split('+').map(str::trim) {
        part_count += 1;
        if part_count > KEYBINDING_CHORD_MAX_PARTS || part.is_empty() {
            return None;
        }

        if key_chord_part_has_unsafe_display_control(part) {
            return None;
        }

        if let Some(modifier) = parse_key_chord_modifier(part) {
            if key.is_some() {
                return None;
            }

            match modifier {
                KeyChordModifier::Ctrl => {
                    if has_ctrl {
                        return None;
                    }
                    has_ctrl = true;
                    modifiers |= Modifiers::CTRL;
                }
                KeyChordModifier::Shift => {
                    if has_shift {
                        return None;
                    }
                    has_shift = true;
                    modifiers |= Modifiers::SHIFT;
                }
                KeyChordModifier::Alt => {
                    if has_alt {
                        return None;
                    }
                    has_alt = true;
                    modifiers |= Modifiers::ALT;
                }
                KeyChordModifier::Command => {
                    if has_command {
                        return None;
                    }
                    has_command = true;
                    modifiers |= Modifiers::COMMAND;
                }
            }
        } else {
            let parsed_key = parse_key_name(part)?;
            if key.replace(parsed_key).is_some() {
                return None;
            }
        }
    }

    key.map(|key| ParsedKeyChord {
        modifiers,
        key,
        has_ctrl,
        has_alt,
        has_shift,
        has_command,
    })
}

fn parse_key_chord_modifier(part: &str) -> Option<KeyChordModifier> {
    if part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control") {
        Some(KeyChordModifier::Ctrl)
    } else if part.eq_ignore_ascii_case("shift") {
        Some(KeyChordModifier::Shift)
    } else if part.eq_ignore_ascii_case("alt") || part.eq_ignore_ascii_case("option") {
        Some(KeyChordModifier::Alt)
    } else if part.eq_ignore_ascii_case("cmd")
        || part.eq_ignore_ascii_case("command")
        || part.eq_ignore_ascii_case("super")
    {
        Some(KeyChordModifier::Command)
    } else {
        None
    }
}

fn parse_key_name(name: &str) -> Option<Key> {
    parse_single_char_key_name(name)
        .or_else(|| parse_function_key_name(name))
        .or_else(|| parse_named_key_name(name))
}

fn parse_single_char_key_name(name: &str) -> Option<Key> {
    if name.len() != 1 {
        return None;
    }

    match name.as_bytes()[0].to_ascii_lowercase() {
        b'a' => Some(Key::A),
        b'b' => Some(Key::B),
        b'c' => Some(Key::C),
        b'd' => Some(Key::D),
        b'e' => Some(Key::E),
        b'f' => Some(Key::F),
        b'g' => Some(Key::G),
        b'h' => Some(Key::H),
        b'i' => Some(Key::I),
        b'j' => Some(Key::J),
        b'k' => Some(Key::K),
        b'l' => Some(Key::L),
        b'm' => Some(Key::M),
        b'n' => Some(Key::N),
        b'o' => Some(Key::O),
        b'p' => Some(Key::P),
        b'q' => Some(Key::Q),
        b'r' => Some(Key::R),
        b's' => Some(Key::S),
        b't' => Some(Key::T),
        b'u' => Some(Key::U),
        b'v' => Some(Key::V),
        b'w' => Some(Key::W),
        b'x' => Some(Key::X),
        b'y' => Some(Key::Y),
        b'z' => Some(Key::Z),
        b'0' => Some(Key::Num0),
        b'1' => Some(Key::Num1),
        b'2' => Some(Key::Num2),
        b'3' => Some(Key::Num3),
        b'4' => Some(Key::Num4),
        b'5' => Some(Key::Num5),
        b'6' => Some(Key::Num6),
        b'7' => Some(Key::Num7),
        b'8' => Some(Key::Num8),
        b'9' => Some(Key::Num9),
        b'`' => Some(Key::Backtick),
        b'\\' => Some(Key::Backslash),
        b'[' => Some(Key::OpenBracket),
        b']' => Some(Key::CloseBracket),
        b',' => Some(Key::Comma),
        b'.' => Some(Key::Period),
        b'/' => Some(Key::Slash),
        b'-' => Some(Key::Minus),
        b'=' => Some(Key::Equals),
        b';' => Some(Key::Semicolon),
        b'\'' => Some(Key::Quote),
        _ => None,
    }
}

fn parse_function_key_name(name: &str) -> Option<Key> {
    let (prefix, number) = name.as_bytes().split_first()?;
    if !prefix.eq_ignore_ascii_case(&b'f') {
        return None;
    }

    match number {
        [b'1'] => Some(Key::F1),
        [b'2'] => Some(Key::F2),
        [b'3'] => Some(Key::F3),
        [b'4'] => Some(Key::F4),
        [b'5'] => Some(Key::F5),
        [b'6'] => Some(Key::F6),
        [b'7'] => Some(Key::F7),
        [b'8'] => Some(Key::F8),
        [b'9'] => Some(Key::F9),
        [b'1', b'0'] => Some(Key::F10),
        [b'1', b'1'] => Some(Key::F11),
        [b'1', b'2'] => Some(Key::F12),
        _ => None,
    }
}

fn parse_named_key_name(name: &str) -> Option<Key> {
    if key_name_matches(name, &["up", "arrowup"]) {
        Some(Key::ArrowUp)
    } else if key_name_matches(name, &["down", "arrowdown"]) {
        Some(Key::ArrowDown)
    } else if key_name_matches(name, &["left", "arrowleft"]) {
        Some(Key::ArrowLeft)
    } else if key_name_matches(name, &["right", "arrowright"]) {
        Some(Key::ArrowRight)
    } else if key_name_matches(name, &["enter"]) {
        Some(Key::Enter)
    } else if key_name_matches(name, &["tab"]) {
        Some(Key::Tab)
    } else if key_name_matches(name, &["space"]) {
        Some(Key::Space)
    } else if key_name_matches(name, &["escape", "esc"]) {
        Some(Key::Escape)
    } else if key_name_matches(name, &["backspace"]) {
        Some(Key::Backspace)
    } else if key_name_matches(name, &["delete", "del"]) {
        Some(Key::Delete)
    } else if key_name_matches(name, &["home"]) {
        Some(Key::Home)
    } else if key_name_matches(name, &["end"]) {
        Some(Key::End)
    } else if key_name_matches(name, &["pageup"]) {
        Some(Key::PageUp)
    } else if key_name_matches(name, &["pagedown"]) {
        Some(Key::PageDown)
    } else if key_name_matches(name, &["backtick"]) {
        Some(Key::Backtick)
    } else if key_name_matches(name, &["backslash"]) {
        Some(Key::Backslash)
    } else if key_name_matches(name, &["openbracket", "leftbracket"]) {
        Some(Key::OpenBracket)
    } else if key_name_matches(name, &["closebracket", "rightbracket"]) {
        Some(Key::CloseBracket)
    } else if key_name_matches(name, &["comma"]) {
        Some(Key::Comma)
    } else if key_name_matches(name, &["period"]) {
        Some(Key::Period)
    } else if key_name_matches(name, &["slash"]) {
        Some(Key::Slash)
    } else if key_name_matches(name, &["minus"]) {
        Some(Key::Minus)
    } else if key_name_matches(name, &["equals"]) {
        Some(Key::Equals)
    } else if key_name_matches(name, &["semicolon"]) {
        Some(Key::Semicolon)
    } else if key_name_matches(name, &["quote"]) {
        Some(Key::Quote)
    } else {
        None
    }
}

fn key_name_matches(name: &str, aliases: &[&str]) -> bool {
    aliases.iter().any(|alias| name.eq_ignore_ascii_case(alias))
}

fn key_chord_exceeds_max_chars(chord: &str) -> bool {
    chord.chars().nth(KEYBINDING_CHORD_MAX_CHARS).is_some()
}

fn key_chord_part_has_unsafe_display_control(part: &str) -> bool {
    part.chars().any(|ch| {
        ch.is_control()
            || matches!(
                ch,
                '\u{061c}'
                    | '\u{200b}'..='\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
                    | '\u{feff}'
            )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        KEYBINDING_CHORD_MAX_CHARS, key_chord_exceeds_max_chars, normalize_key_chord,
        parse_key_chord,
    };

    #[test]
    fn normalize_key_chord_accepts_mixed_case_modifiers_without_reordering_key() {
        assert_eq!(
            normalize_key_chord("cTrL + ShIfT + P"),
            Some("Ctrl+Shift+P".to_owned())
        );
    }

    #[test]
    fn normalize_key_chord_rejects_duplicate_mixed_case_modifier() {
        assert_eq!(normalize_key_chord("Ctrl + CONTROL + P"), None);
    }

    #[test]
    fn normalize_key_chord_rejects_hidden_controls_and_overlong_chords() {
        assert_eq!(normalize_key_chord("Ctrl+\u{202e}P"), None);
        assert_eq!(normalize_key_chord("Ctrl+\u{2066}P"), None);
        assert_eq!(normalize_key_chord("Ctrl+Sh\tift+P"), None);

        let long_chord = format!("Ctrl+{}", "P".repeat(KEYBINDING_CHORD_MAX_CHARS));
        assert_eq!(normalize_key_chord(&long_chord), None);
    }

    #[test]
    fn normalize_key_chord_applies_display_limit_before_stale_tail() {
        let exact_limit = format!(
            "Ctrl+P{}",
            " ".repeat(KEYBINDING_CHORD_MAX_CHARS - "Ctrl+P".chars().count())
        );
        assert_eq!(exact_limit.chars().count(), KEYBINDING_CHORD_MAX_CHARS);
        assert!(!key_chord_exceeds_max_chars(&exact_limit));
        assert_eq!(normalize_key_chord(&exact_limit), Some("Ctrl+P".to_owned()));

        let over_limit = format!("{exact_limit}+");
        assert!(key_chord_exceeds_max_chars(&over_limit));
        assert_eq!(normalize_key_chord(&over_limit), None);

        let stale_pending_chord = format!("Ctrl+P{}", "+".repeat(KEYBINDING_CHORD_MAX_CHARS * 8));
        assert_eq!(normalize_key_chord(&stale_pending_chord), None);
    }

    #[test]
    fn normalize_key_chord_rejects_duplicated_separators() {
        for chord in ["Ctrl++P", "Ctrl+ +P", "+Ctrl+P", "Ctrl+P+", "Ctrl++++P"] {
            assert_eq!(normalize_key_chord(chord), None, "{chord:?}");
        }
    }

    #[test]
    fn normalize_key_chord_rejects_modifiers_after_key() {
        for chord in ["P+Ctrl", "Space+Alt", "F1+Command", "Left+Shift"] {
            assert_eq!(normalize_key_chord(chord), None, "{chord:?}");
            assert!(parse_key_chord(chord).is_none(), "{chord:?}");
        }

        assert_eq!(
            normalize_key_chord("Ctrl+Alt+Shift+Cmd+P"),
            Some("Ctrl+Alt+Shift+Cmd+P".to_owned())
        );
    }
}
