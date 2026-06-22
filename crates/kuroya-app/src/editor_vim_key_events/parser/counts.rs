use eframe::egui::{Key, Modifiers};

use super::super::VIM_MAX_COUNT;

pub(in crate::editor_vim_key_events) fn vim_count_digit(
    key: Key,
    modifiers: Modifiers,
    allow_zero: bool,
) -> Option<usize> {
    if modifiers.command || modifiers.alt || modifiers.ctrl || modifiers.shift {
        return None;
    }
    match key {
        Key::Num0 if allow_zero => Some(0),
        Key::Num1 => Some(1),
        Key::Num2 => Some(2),
        Key::Num3 => Some(3),
        Key::Num4 => Some(4),
        Key::Num5 => Some(5),
        Key::Num6 => Some(6),
        Key::Num7 => Some(7),
        Key::Num8 => Some(8),
        Key::Num9 => Some(9),
        _ => None,
    }
}

pub(in crate::editor_vim_key_events) fn vim_push_count_digit(count: usize, digit: usize) -> usize {
    count
        .saturating_mul(10)
        .saturating_add(digit)
        .clamp(1, VIM_MAX_COUNT)
}

pub(in crate::editor_vim_key_events) fn vim_register_command_count(
    prefix_count: usize,
    command_count: Option<usize>,
) -> usize {
    match command_count {
        Some(command_count) => vim_combined_count(prefix_count, command_count),
        None => prefix_count.clamp(1, VIM_MAX_COUNT),
    }
}

pub(in crate::editor_vim_key_events) fn vim_register_command_next_count(
    command_count: Option<usize>,
    key: Key,
    modifiers: Modifiers,
) -> Option<usize> {
    match command_count {
        Some(count) => {
            vim_count_digit(key, modifiers, true).map(|digit| vim_push_count_digit(count, digit))
        }
        None => vim_count_digit(key, modifiers, false),
    }
}

pub(in crate::editor_vim_key_events) fn vim_combined_count(
    operator_count: usize,
    motion_count: usize,
) -> usize {
    operator_count
        .max(1)
        .saturating_mul(motion_count.max(1))
        .clamp(1, VIM_MAX_COUNT)
}
