use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::super::*;

pub(super) fn handle_vim_convert_case_pending_key_event(
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
        (
            EditorVimPendingKey::ConvertCaseOperator {
                operator_count,
                conversion,
            },
            key,
        ) => {
            if vim_case_conversion_repeated_operator_key(conversion, key, modifiers) {
                let count = vim_combined_count(operator_count, 1);
                return Some(vim_repeatable_change_result(
                    vim_convert_case_lines(buffer, count, conversion),
                    last_change,
                    EditorVimRepeatAction::ConvertCaseLines(conversion),
                    count,
                    suppress_text,
                ));
            }
            if key == Key::U && !modifiers.command && !modifiers.alt && !modifiers.ctrl {
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(digit) = vim_count_digit(key, modifiers, false) {
                *pending = Some(EditorVimPendingKey::ConvertCaseMotionCount {
                    operator_count,
                    motion_count: digit,
                    conversion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ConvertCaseTextObject {
                    operator_count,
                    motion_count: 1,
                    scope,
                    conversion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ConvertCaseCharFind {
                    operator_count,
                    motion_count: 1,
                    motion,
                    conversion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                let count = vim_combined_count(operator_count, 1);
                return Some(vim_repeatable_change_result(
                    vim_convert_case_operator_motion(buffer, operator_count, 1, motion, conversion),
                    last_change,
                    EditorVimRepeatAction::ConvertCaseOperatorMotion { motion, conversion },
                    count,
                    suppress_text,
                ));
            }
            None
        }
        (
            EditorVimPendingKey::ConvertCaseMotionCount {
                operator_count,
                motion_count,
                conversion,
            },
            key,
        ) => {
            if vim_case_conversion_repeated_operator_key(conversion, key, modifiers) {
                let count = vim_combined_count(operator_count, motion_count);
                return Some(vim_repeatable_change_result(
                    vim_convert_case_lines(buffer, count, conversion),
                    last_change,
                    EditorVimRepeatAction::ConvertCaseLines(conversion),
                    count,
                    suppress_text,
                ));
            }
            if key == Key::U && !modifiers.command && !modifiers.alt && !modifiers.ctrl {
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(digit) = vim_count_digit(key, modifiers, true) {
                *pending = Some(EditorVimPendingKey::ConvertCaseMotionCount {
                    operator_count,
                    motion_count: vim_push_count_digit(motion_count, digit),
                    conversion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ConvertCaseTextObject {
                    operator_count,
                    motion_count,
                    scope,
                    conversion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                *pending = Some(EditorVimPendingKey::ConvertCaseCharFind {
                    operator_count,
                    motion_count,
                    motion,
                    conversion,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
            if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                let count = vim_combined_count(operator_count, motion_count);
                return Some(vim_repeatable_change_result(
                    vim_convert_case_operator_motion(
                        buffer,
                        operator_count,
                        motion_count,
                        motion,
                        conversion,
                    ),
                    last_change,
                    EditorVimRepeatAction::ConvertCaseOperatorMotion { motion, conversion },
                    count,
                    suppress_text,
                ));
            }
            None
        }
        (
            EditorVimPendingKey::ConvertCaseCharFind {
                operator_count,
                motion_count,
                motion,
                conversion,
            },
            _,
        ) => {
            let target = vim_printable_key_char(key, modifiers)?;
            let count = vim_combined_count(operator_count, motion_count);
            let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
            *last_char_find = Some(EditorVimCharFind { motion, target });
            Some(vim_repeatable_change_result(
                vim_convert_case_operator_motion(
                    buffer,
                    operator_count,
                    motion_count,
                    operator_motion,
                    conversion,
                ),
                last_change,
                EditorVimRepeatAction::ConvertCaseOperatorMotion {
                    motion: operator_motion,
                    conversion,
                },
                count,
                suppress_text,
            ))
        }
        (
            EditorVimPendingKey::ConvertCaseTextObject {
                operator_count,
                motion_count,
                scope,
                conversion,
            },
            key,
        ) => {
            let kind = vim_text_object_kind_for_key(key, modifiers)?;
            let count = vim_combined_count(operator_count, motion_count);
            Some(vim_repeatable_change_result(
                vim_convert_case_text_object(
                    buffer,
                    operator_count,
                    motion_count,
                    scope,
                    kind,
                    conversion,
                ),
                last_change,
                EditorVimRepeatAction::ConvertCaseTextObject {
                    scope,
                    kind,
                    conversion,
                },
                count,
                suppress_text,
            ))
        }
        _ => None,
    }
}
