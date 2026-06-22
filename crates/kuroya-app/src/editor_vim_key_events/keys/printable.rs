use eframe::egui::{Key, Modifiers};

use super::modifiers::no_text_modifiers;

pub(in crate::editor_vim_key_events) fn vim_printable_key_char(
    key: Key,
    modifiers: Modifiers,
) -> Option<char> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    let shifted = modifiers.shift;
    match key {
        Key::A => Some(if shifted { 'A' } else { 'a' }),
        Key::B => Some(if shifted { 'B' } else { 'b' }),
        Key::C => Some(if shifted { 'C' } else { 'c' }),
        Key::Colon => Some(':'),
        Key::Comma => Some(if shifted { '<' } else { ',' }),
        Key::D => Some(if shifted { 'D' } else { 'd' }),
        Key::E => Some(if shifted { 'E' } else { 'e' }),
        Key::Equals => Some(if shifted { '+' } else { '=' }),
        Key::Exclamationmark => Some('!'),
        Key::F => Some(if shifted { 'F' } else { 'f' }),
        Key::G => Some(if shifted { 'G' } else { 'g' }),
        Key::H => Some(if shifted { 'H' } else { 'h' }),
        Key::I => Some(if shifted { 'I' } else { 'i' }),
        Key::J => Some(if shifted { 'J' } else { 'j' }),
        Key::K => Some(if shifted { 'K' } else { 'k' }),
        Key::L => Some(if shifted { 'L' } else { 'l' }),
        Key::M => Some(if shifted { 'M' } else { 'm' }),
        Key::Minus => Some(if shifted { '_' } else { '-' }),
        Key::N => Some(if shifted { 'N' } else { 'n' }),
        Key::O => Some(if shifted { 'O' } else { 'o' }),
        Key::P => Some(if shifted { 'P' } else { 'p' }),
        Key::Q => Some(if shifted { 'Q' } else { 'q' }),
        Key::Period => Some(if shifted { '>' } else { '.' }),
        Key::OpenBracket => Some(if shifted { '{' } else { '[' }),
        Key::CloseBracket => Some(if shifted { '}' } else { ']' }),
        Key::OpenCurlyBracket => Some('{'),
        Key::CloseCurlyBracket => Some('}'),
        Key::Plus => Some('+'),
        Key::Questionmark => Some('?'),
        Key::R => Some(if shifted { 'R' } else { 'r' }),
        Key::S => Some(if shifted { 'S' } else { 's' }),
        Key::Semicolon => Some(if shifted { ':' } else { ';' }),
        Key::Slash => Some(if shifted { '?' } else { '/' }),
        Key::Space => Some(' '),
        Key::T => Some(if shifted { 'T' } else { 't' }),
        Key::U => Some(if shifted { 'U' } else { 'u' }),
        Key::V => Some(if shifted { 'V' } else { 'v' }),
        Key::W => Some(if shifted { 'W' } else { 'w' }),
        Key::X => Some(if shifted { 'X' } else { 'x' }),
        Key::Y => Some(if shifted { 'Y' } else { 'y' }),
        Key::Z => Some(if shifted { 'Z' } else { 'z' }),
        Key::Backslash => Some(if shifted { '|' } else { '\\' }),
        Key::Backtick => Some(if shifted { '~' } else { '`' }),
        Key::Pipe => Some('|'),
        Key::Quote => Some(if shifted { '"' } else { '\'' }),
        Key::Num0 if !shifted => Some('0'),
        Key::Num1 if shifted => Some('!'),
        Key::Num1 if !shifted => Some('1'),
        Key::Num2 if shifted => Some('@'),
        Key::Num2 if !shifted => Some('2'),
        Key::Num3 if !shifted => Some('3'),
        Key::Num3 if shifted => Some('#'),
        Key::Num4 if !shifted => Some('4'),
        Key::Num4 if shifted => Some('$'),
        Key::Num5 if !shifted => Some('5'),
        Key::Num5 if shifted => Some('%'),
        Key::Num6 if !shifted => Some('6'),
        Key::Num6 if shifted => Some('^'),
        Key::Num7 if shifted => Some('&'),
        Key::Num7 if !shifted => Some('7'),
        Key::Num8 if !shifted => Some('8'),
        Key::Num8 if shifted => Some('*'),
        Key::Num9 if !shifted => Some('9'),
        Key::Num9 if shifted => Some('('),
        Key::Num0 if shifted => Some(')'),
        _ => None,
    }
}

pub(in crate::editor_vim_key_events) fn vim_replacement_key_char(
    key: Key,
    modifiers: Modifiers,
) -> Option<char> {
    if key == Key::Enter && no_text_modifiers(modifiers) {
        Some('\n')
    } else {
        vim_printable_key_char(key, modifiers)
    }
}
