use kuroya_core::EditorCursorStyle;

use super::{EditorVimMode, EditorVimPendingKey};

mod labels;
mod pending;

pub(crate) use self::pending::{
    vim_pending_command_status_label, vim_pending_key_sequence_status_label,
};
use self::pending::{vim_pending_is_replace, vim_pending_is_visual};

pub(crate) fn vim_mode_status_label(
    mode: EditorVimMode,
    pending: Option<EditorVimPendingKey>,
) -> &'static str {
    if pending.is_some_and(vim_pending_is_replace) {
        return "REPLACE";
    }
    if pending.is_some_and(vim_pending_is_visual) {
        return "VISUAL";
    }
    match mode {
        EditorVimMode::Normal => "NORMAL",
        EditorVimMode::Insert => "INSERT",
    }
}

pub(crate) fn vim_effective_cursor_style(
    configured: EditorCursorStyle,
    vim_keybindings: bool,
    mode: EditorVimMode,
    pending: Option<EditorVimPendingKey>,
) -> EditorCursorStyle {
    if !vim_keybindings {
        return configured;
    }
    if pending.is_some_and(vim_pending_is_visual) {
        return EditorCursorStyle::Block;
    }
    match mode {
        EditorVimMode::Normal => EditorCursorStyle::Block,
        EditorVimMode::Insert => EditorCursorStyle::LineThin,
    }
}
