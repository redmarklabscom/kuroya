use eframe::egui::{Key, Modifiers};

use super::super::vim_printable_key_char;
use super::shared::vim_ctrl_key_char;

pub(crate) fn vim_key_token_for_event(key: Key, modifiers: Modifiers) -> Option<String> {
    if modifiers.command || modifiers.alt {
        return None;
    }
    if modifiers.ctrl {
        return vim_ctrl_key_token_for_event(key, modifiers);
    }
    if let Some(ch) = vim_printable_key_char(key, modifiers) {
        return Some(ch.to_string());
    }
    match key {
        Key::Escape => Some("<Esc>".to_owned()),
        Key::Enter => Some("<Enter>".to_owned()),
        Key::Tab => Some("<Tab>".to_owned()),
        Key::Backspace => Some("<Backspace>".to_owned()),
        Key::Delete => Some("<Delete>".to_owned()),
        Key::Home => Some("<Home>".to_owned()),
        Key::End => Some("<End>".to_owned()),
        Key::ArrowLeft => Some("<Left>".to_owned()),
        Key::ArrowRight => Some("<Right>".to_owned()),
        Key::ArrowUp => Some("<Up>".to_owned()),
        Key::ArrowDown => Some("<Down>".to_owned()),
        _ => None,
    }
}

fn vim_ctrl_key_token_for_event(key: Key, modifiers: Modifiers) -> Option<String> {
    if modifiers.shift {
        return None;
    }
    if key == Key::OpenBracket {
        return Some("<Esc>".to_owned());
    }
    let ch = vim_ctrl_key_char(key)?;
    Some(format!("<C-{ch}>"))
}
