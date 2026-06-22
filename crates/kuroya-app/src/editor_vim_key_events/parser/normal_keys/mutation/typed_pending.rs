use eframe::egui::{Key, Modifiers};

use super::super::super::super::{
    EditorVimPendingKey, vim_command_input_accept_key, vim_replacement_key_char,
};

pub(super) fn vim_typed_pending_key_can_mutate(
    key: Key,
    modifiers: Modifiers,
    pending: Option<EditorVimPendingKey>,
    printable_key_char: Option<char>,
) -> Option<bool> {
    if matches!(pending, Some(EditorVimPendingKey::SearchInput { .. })) {
        return Some(false);
    }
    if matches!(pending, Some(EditorVimPendingKey::CommandInput)) {
        return Some(vim_command_input_accept_key(key, modifiers));
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::Go(_)), Key::Backtick, true)
            | (Some(EditorVimPendingKey::Go(_)), Key::U, false)
            | (Some(EditorVimPendingKey::Go(_)), Key::U, true)
    ) {
        return Some(false);
    }
    if matches!(pending, Some(EditorVimPendingKey::ReplaceChar(_))) {
        return Some(vim_replacement_key_char(key, modifiers).is_some());
    }
    if matches!(pending, Some(EditorVimPendingKey::RegisterCommand { .. })) {
        return Some(
            key == Key::P
                || key == Key::S
                || key == Key::X
                || matches!(key, Key::C | Key::D) && modifiers.shift,
        );
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::FindCharForward(_)
                | EditorVimPendingKey::FindCharBackward(_)
                | EditorVimPendingKey::TillCharForward(_)
                | EditorVimPendingKey::TillCharBackward(_)
        )
    ) && printable_key_char.is_some()
    {
        return Some(false);
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeCharFind { .. }
                | EditorVimPendingKey::ChangeCharFindIntoRegister { .. }
                | EditorVimPendingKey::DeleteCharFind { .. }
                | EditorVimPendingKey::DeleteCharFindIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseCharFind { .. }
                | EditorVimPendingKey::ToggleCaseCharFind { .. }
        )
    ) && printable_key_char.is_some()
    {
        return Some(true);
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::YankCharFind { .. }
                | EditorVimPendingKey::YankCharFindIntoRegister { .. }
        )
    ) && printable_key_char.is_some()
    {
        return Some(false);
    }
    None
}
