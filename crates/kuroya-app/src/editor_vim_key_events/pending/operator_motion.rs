use eframe::egui::{Key, Modifiers};

use super::super::{
    EditorVimCharFindMotion, EditorVimNamedRegister, EditorVimPendingKey, EditorVimTextObjectScope,
    VimKeyResult, vim_count_digit, vim_operator_char_find_motion_for_key, vim_push_count_digit,
    vim_text_object_scope_for_key,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::editor_vim_key_events::pending) enum VimPendingOperator {
    Change,
    Delete,
    Yank,
}

pub(in crate::editor_vim_key_events::pending) fn handle_vim_pending_operator_motion_transition(
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    operator: VimPendingOperator,
    register: Option<EditorVimNamedRegister>,
    operator_count: usize,
    motion_count: usize,
    continue_motion_count: bool,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if let Some(digit) = vim_count_digit(key, modifiers, continue_motion_count) {
        let motion_count = if continue_motion_count {
            vim_push_count_digit(motion_count, digit)
        } else {
            digit
        };
        *pending = Some(operator.motion_count_pending(operator_count, motion_count, register));
        return Some(VimKeyResult::handled(suppress_text));
    }
    if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
        *pending =
            Some(operator.text_object_pending(operator_count, motion_count, scope, register));
        return Some(VimKeyResult::handled(suppress_text));
    }
    if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
        *pending = Some(operator.char_find_pending(operator_count, motion_count, motion, register));
        return Some(VimKeyResult::handled(suppress_text));
    }
    None
}

impl VimPendingOperator {
    fn motion_count_pending(
        self,
        operator_count: usize,
        motion_count: usize,
        register: Option<EditorVimNamedRegister>,
    ) -> EditorVimPendingKey {
        match (self, register) {
            (Self::Change, Some(register)) => EditorVimPendingKey::ChangeMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            },
            (Self::Change, None) => EditorVimPendingKey::ChangeMotionCount {
                operator_count,
                motion_count,
            },
            (Self::Delete, Some(register)) => EditorVimPendingKey::DeleteMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            },
            (Self::Delete, None) => EditorVimPendingKey::DeleteMotionCount {
                operator_count,
                motion_count,
            },
            (Self::Yank, Some(register)) => EditorVimPendingKey::YankMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            },
            (Self::Yank, None) => EditorVimPendingKey::YankMotionCount {
                operator_count,
                motion_count,
            },
        }
    }

    fn text_object_pending(
        self,
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
        register: Option<EditorVimNamedRegister>,
    ) -> EditorVimPendingKey {
        match (self, register) {
            (Self::Change, Some(register)) => EditorVimPendingKey::ChangeTextObjectIntoRegister {
                operator_count,
                motion_count,
                scope,
                register,
            },
            (Self::Change, None) => EditorVimPendingKey::ChangeTextObject {
                operator_count,
                motion_count,
                scope,
            },
            (Self::Delete, Some(register)) => EditorVimPendingKey::DeleteTextObjectIntoRegister {
                operator_count,
                motion_count,
                scope,
                register,
            },
            (Self::Delete, None) => EditorVimPendingKey::DeleteTextObject {
                operator_count,
                motion_count,
                scope,
            },
            (Self::Yank, Some(register)) => EditorVimPendingKey::YankTextObjectIntoRegister {
                operator_count,
                motion_count,
                scope,
                register,
            },
            (Self::Yank, None) => EditorVimPendingKey::YankTextObject {
                operator_count,
                motion_count,
                scope,
            },
        }
    }

    fn char_find_pending(
        self,
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
        register: Option<EditorVimNamedRegister>,
    ) -> EditorVimPendingKey {
        match (self, register) {
            (Self::Change, Some(register)) => EditorVimPendingKey::ChangeCharFindIntoRegister {
                operator_count,
                motion_count,
                motion,
                register,
            },
            (Self::Change, None) => EditorVimPendingKey::ChangeCharFind {
                operator_count,
                motion_count,
                motion,
            },
            (Self::Delete, Some(register)) => EditorVimPendingKey::DeleteCharFindIntoRegister {
                operator_count,
                motion_count,
                motion,
                register,
            },
            (Self::Delete, None) => EditorVimPendingKey::DeleteCharFind {
                operator_count,
                motion_count,
                motion,
            },
            (Self::Yank, Some(register)) => EditorVimPendingKey::YankCharFindIntoRegister {
                operator_count,
                motion_count,
                motion,
                register,
            },
            (Self::Yank, None) => EditorVimPendingKey::YankCharFind {
                operator_count,
                motion_count,
                motion,
            },
        }
    }
}
