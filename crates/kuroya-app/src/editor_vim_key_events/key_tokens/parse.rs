use eframe::egui::{Key, Modifiers};

use super::shared::{matches_ignore_ascii_case, vim_ctrl_key_char, vim_letter_key};

pub(in crate::editor_vim_key_events) fn vim_key_sequence_events(
    sequence: &str,
) -> Option<Vec<(Key, Modifiers)>> {
    let mut keys = Vec::new();
    let chars: Vec<char> = sequence.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '<' {
            let close_index = chars[index + 1..]
                .iter()
                .position(|token_ch| *token_ch == '>')
                .map(|offset| index + 1 + offset);
            if let Some(close_index) = close_index {
                let token: String = chars[index + 1..close_index].iter().collect();
                if !token.is_empty() {
                    keys.push(vim_key_event_for_named_token(&token)?);
                    index = close_index + 1;
                    continue;
                }
            }
        }
        keys.push(vim_key_event_for_char(ch)?);
        index += 1;
    }
    Some(keys)
}

fn vim_key_event_for_named_token(token: &str) -> Option<(Key, Modifiers)> {
    if let Some(event) = vim_modified_key_event_for_named_token(token) {
        return Some(event);
    }
    let modifiers = Modifiers::NONE;
    if token.eq_ignore_ascii_case("esc") || token.eq_ignore_ascii_case("escape") {
        Some((Key::Escape, modifiers))
    } else if token.eq_ignore_ascii_case("cr") || token.eq_ignore_ascii_case("enter") {
        Some((Key::Enter, modifiers))
    } else if token.eq_ignore_ascii_case("tab") {
        Some((Key::Tab, modifiers))
    } else if token.eq_ignore_ascii_case("space") {
        Some((Key::Space, modifiers))
    } else if token.eq_ignore_ascii_case("bs") || token.eq_ignore_ascii_case("backspace") {
        Some((Key::Backspace, modifiers))
    } else if token.eq_ignore_ascii_case("del") || token.eq_ignore_ascii_case("delete") {
        Some((Key::Delete, modifiers))
    } else if token.eq_ignore_ascii_case("home") {
        Some((Key::Home, modifiers))
    } else if token.eq_ignore_ascii_case("end") {
        Some((Key::End, modifiers))
    } else if token.eq_ignore_ascii_case("left") {
        Some((Key::ArrowLeft, modifiers))
    } else if token.eq_ignore_ascii_case("right") {
        Some((Key::ArrowRight, modifiers))
    } else if token.eq_ignore_ascii_case("up") {
        Some((Key::ArrowUp, modifiers))
    } else if token.eq_ignore_ascii_case("down") {
        Some((Key::ArrowDown, modifiers))
    } else {
        None
    }
}

fn vim_modified_key_event_for_named_token(token: &str) -> Option<(Key, Modifiers)> {
    let (modifier, key_token) = token.split_once('-')?;
    if !matches_ignore_ascii_case(modifier, &["c", "ctrl", "control"]) {
        return None;
    }
    let mut chars = key_token.chars();
    let key_char = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let key_char = key_char.to_ascii_lowercase();
    let key = vim_letter_key(key_char);
    if vim_ctrl_key_char(key) != Some(key_char) {
        return None;
    }
    let mut modifiers = Modifiers::NONE;
    modifiers.ctrl = true;
    Some((key, modifiers))
}

fn vim_key_event_for_char(ch: char) -> Option<(Key, Modifiers)> {
    let mut modifiers = Modifiers::NONE;
    let key = match ch {
        'a'..='z' => vim_letter_key(ch),
        'A'..='Z' => {
            modifiers.shift = true;
            vim_letter_key(ch.to_ascii_lowercase())
        }
        '0' => Key::Num0,
        ')' => {
            modifiers.shift = true;
            Key::Num0
        }
        '1' => Key::Num1,
        '!' => {
            modifiers.shift = true;
            Key::Num1
        }
        '2' => Key::Num2,
        '@' => {
            modifiers.shift = true;
            Key::Num2
        }
        '3' => Key::Num3,
        '#' => {
            modifiers.shift = true;
            Key::Num3
        }
        '4' => Key::Num4,
        '$' => {
            modifiers.shift = true;
            Key::Num4
        }
        '5' => Key::Num5,
        '%' => {
            modifiers.shift = true;
            Key::Num5
        }
        '6' => Key::Num6,
        '^' => {
            modifiers.shift = true;
            Key::Num6
        }
        '7' => Key::Num7,
        '&' => {
            modifiers.shift = true;
            Key::Num7
        }
        '8' => Key::Num8,
        '*' => {
            modifiers.shift = true;
            Key::Num8
        }
        '9' => Key::Num9,
        '(' => {
            modifiers.shift = true;
            Key::Num9
        }
        ',' => Key::Comma,
        '<' => {
            modifiers.shift = true;
            Key::Comma
        }
        '.' => Key::Period,
        '>' => {
            modifiers.shift = true;
            Key::Period
        }
        '-' => Key::Minus,
        '_' => {
            modifiers.shift = true;
            Key::Minus
        }
        '=' => Key::Equals,
        '+' => {
            modifiers.shift = true;
            Key::Equals
        }
        '[' => Key::OpenBracket,
        '{' => {
            modifiers.shift = true;
            Key::OpenBracket
        }
        ']' => Key::CloseBracket,
        '}' => {
            modifiers.shift = true;
            Key::CloseBracket
        }
        ';' => Key::Semicolon,
        ':' => Key::Colon,
        '/' => Key::Slash,
        '?' => {
            modifiers.shift = true;
            Key::Slash
        }
        '\\' => Key::Backslash,
        '|' => {
            modifiers.shift = true;
            Key::Backslash
        }
        '`' => Key::Backtick,
        '~' => {
            modifiers.shift = true;
            Key::Backtick
        }
        '\'' => Key::Quote,
        '"' => {
            modifiers.shift = true;
            Key::Quote
        }
        ' ' => Key::Space,
        _ => return None,
    };
    Some((key, modifiers))
}
