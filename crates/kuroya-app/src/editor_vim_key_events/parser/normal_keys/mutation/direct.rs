use eframe::egui::{Key, Modifiers};

pub(super) fn vim_direct_normal_key_can_mutate(key: Key, modifiers: Modifiers) -> bool {
    key == Key::Period && !modifiers.shift
        || matches!(key, Key::O | Key::P | Key::S | Key::X | Key::U)
        || key == Key::Backtick && modifiers.shift
        || key == Key::J && modifiers.shift
        || matches!(key, Key::C | Key::D) && modifiers.shift
}
