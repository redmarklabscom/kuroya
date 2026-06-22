use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::*;

pub(super) fn handle_vim_toggle_case_pending_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    last_change: &mut Option<EditorVimLastChange>,
    pending_key: EditorVimPendingKey,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    match (pending_key, key) {
        (EditorVimPendingKey::ToggleCaseOperator(operator_count), key) => {
            if let Some(digit) = vim_count_digit(key, modifiers, false) {
                *pending = Some(EditorVimPendingKey::ToggleCaseMotionCount {
                    operator_count,
                    motion_count: digit,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ToggleCaseTextObject {
                    operator_count,
                    motion_count: 1,
                    scope,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ToggleCaseCharFind {
                    operator_count,
                    motion_count: 1,
                    motion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                let count = vim_combined_count(operator_count, 1);
                return Some(vim_repeatable_change_result(
                    vim_toggle_case_operator_motion(buffer, operator_count, 1, motion),
                    last_change,
                    EditorVimRepeatAction::ToggleCaseOperatorMotion(motion),
                    count,
                    suppress_text,
                ));
            }
            None
        }
        (
            EditorVimPendingKey::ToggleCaseMotionCount {
                operator_count,
                motion_count,
            },
            key,
        ) => {
            if let Some(digit) = vim_count_digit(key, modifiers, true) {
                *pending = Some(EditorVimPendingKey::ToggleCaseMotionCount {
                    operator_count,
                    motion_count: vim_push_count_digit(motion_count, digit),
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ToggleCaseTextObject {
                    operator_count,
                    motion_count,
                    scope,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ToggleCaseCharFind {
                    operator_count,
                    motion_count,
                    motion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                let count = vim_combined_count(operator_count, motion_count);
                return Some(vim_repeatable_change_result(
                    vim_toggle_case_operator_motion(buffer, operator_count, motion_count, motion),
                    last_change,
                    EditorVimRepeatAction::ToggleCaseOperatorMotion(motion),
                    count,
                    suppress_text,
                ));
            }
            None
        }
        (
            EditorVimPendingKey::ToggleCaseCharFind {
                operator_count,
                motion_count,
                motion,
            },
            _,
        ) => {
            let target = vim_printable_key_char(key, modifiers)?;
            let count = vim_combined_count(operator_count, motion_count);
            let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
            *last_char_find = Some(EditorVimCharFind { motion, target });
            Some(vim_repeatable_change_result(
                vim_toggle_case_operator_motion(
                    buffer,
                    operator_count,
                    motion_count,
                    operator_motion,
                ),
                last_change,
                EditorVimRepeatAction::ToggleCaseOperatorMotion(operator_motion),
                count,
                suppress_text,
            ))
        }
        (
            EditorVimPendingKey::ToggleCaseTextObject {
                operator_count,
                motion_count,
                scope,
            },
            key,
        ) => {
            let kind = vim_text_object_kind_for_key(key, modifiers)?;
            let count = vim_combined_count(operator_count, motion_count);
            Some(vim_repeatable_change_result(
                vim_toggle_case_text_object(buffer, operator_count, motion_count, scope, kind),
                last_change,
                EditorVimRepeatAction::ToggleCaseTextObject { scope, kind },
                count,
                suppress_text,
            ))
        }
        _ => None,
    }
}
