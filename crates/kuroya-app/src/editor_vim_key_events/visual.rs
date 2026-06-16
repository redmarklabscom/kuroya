use eframe::egui::{Key, Modifiers};
use kuroya_core::{TextBuffer, TextEdit};
use std::ops::Range;

use super::{
    EditorVimCaseConversion, EditorVimCharFind, EditorVimCharFindMotion, EditorVimLastChange,
    EditorVimMode, EditorVimNamedRegister, EditorVimPendingKey, EditorVimRegister,
    EditorVimRegisterKind, EditorVimRepeatAction, EditorVimTextObjectScope, VIM_MAX_COUNT,
    VimKeyResult, no_text_modifiers, vim_apply_char_find, vim_convert_case_range, vim_count_digit,
    vim_delete_range_into_register, vim_escape_key, vim_line_column_motion_key,
    vim_line_outdent_len, vim_move_counted_line_first_non_whitespace,
    vim_move_next_line_first_non_whitespace, vim_move_next_paragraph,
    vim_move_previous_line_first_non_whitespace, vim_move_previous_paragraph,
    vim_move_space_backward, vim_move_space_forward, vim_move_to_line_column,
    vim_move_to_matching_bracket, vim_named_register_for_key,
    vim_operator_char_find_motion_for_key, vim_operator_motion_for_key, vim_push_count_digit,
    vim_repeat_last_search, vim_repeatable_change_result, vim_search_word_under_cursor,
    vim_text_object_kind_for_key, vim_text_object_range, vim_text_object_scope_for_key,
    vim_yank_range_into_register,
};

pub(super) fn handle_vim_visual_character_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    indent_unit: &str,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    if vim_escape_key(key, modifiers) || vim_visual_character_toggle_key(key, modifiers) {
        *pending = None;
        buffer.set_single_cursor(cursor);
        return VimKeyResult::handled(suppress_text);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        vim_restore_visual_character_pending(pending, anchor, cursor, count);
        return VimKeyResult::ignored();
    }
    if count.is_none()
        && let Some(digit) = vim_count_digit(key, modifiers, false)
    {
        *pending = Some(EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count: digit,
        });
        return VimKeyResult::handled(suppress_text);
    }
    if let Some(count) = count
        && let Some(digit) = vim_count_digit(key, modifiers, true)
    {
        *pending = Some(EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count: vim_push_count_digit(count, digit),
        });
        return VimKeyResult::handled(suppress_text);
    }
    if key == Key::Quote && modifiers.shift {
        *pending = Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
            anchor,
            cursor,
            count,
        });
        return VimKeyResult::handled(suppress_text);
    }
    if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
        *pending = Some(EditorVimPendingKey::VisualCharacterTextObject {
            anchor,
            cursor,
            count,
            scope,
        });
        return VimKeyResult::handled(suppress_text);
    }
    if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
        *pending = Some(EditorVimPendingKey::VisualCharacterCharFind {
            anchor,
            cursor,
            count,
            motion,
        });
        return VimKeyResult::handled(suppress_text);
    }
    if vim_visual_character_swap_key(key, modifiers) {
        vim_set_visual_character_selection(buffer, cursor, anchor);
        *pending = Some(EditorVimPendingKey::VisualCharacter {
            anchor: cursor,
            cursor: anchor,
        });
        return VimKeyResult::handled(suppress_text);
    }
    if key == Key::G && !modifiers.shift {
        *pending = Some(EditorVimPendingKey::VisualCharacterGo {
            anchor,
            cursor,
            count,
        });
        return VimKeyResult::handled(suppress_text);
    }
    if vim_visual_character_yank_key(key, modifiers) {
        vim_yank_visual_character(buffer, anchor, cursor, unnamed_register);
        *pending = None;
        return VimKeyResult::handled(suppress_text);
    }
    if vim_visual_character_join_key(key, modifiers) {
        let repeat_count = vim_visual_character_join_repeat_count(buffer, anchor, cursor);
        let changed = vim_join_visual_character_lines(buffer, anchor, cursor);
        *pending = None;
        return vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::JoinLines,
            repeat_count,
            suppress_text,
        );
    }
    if vim_visual_character_indent_key(key, modifiers) {
        let repeat_count = vim_visual_character_line_repeat_count(buffer, anchor, cursor);
        let changed = vim_indent_visual_character_lines(buffer, anchor, cursor, indent_unit);
        *pending = None;
        return vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::IndentLines,
            repeat_count,
            suppress_text,
        );
    }
    if vim_visual_character_outdent_key(key, modifiers) {
        let repeat_count = vim_visual_character_line_repeat_count(buffer, anchor, cursor);
        let changed = vim_outdent_visual_character_lines(buffer, anchor, cursor, indent_unit);
        *pending = None;
        return vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::OutdentLines,
            repeat_count,
            suppress_text,
        );
    }
    if let Some(conversion) = vim_visual_character_case_conversion(key, modifiers) {
        let repeat_count = vim_visual_character_repeat_count(buffer, anchor, cursor);
        let changed = vim_convert_case_visual_character(buffer, anchor, cursor, conversion);
        *pending = None;
        return vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::ConvertCaseForwardChars(conversion),
            repeat_count,
            suppress_text,
        );
    }
    if vim_visual_character_delete_key(key, modifiers) {
        let changed = vim_delete_visual_character(buffer, anchor, cursor, unnamed_register);
        *pending = None;
        return if changed {
            VimKeyResult::changed(suppress_text)
        } else {
            VimKeyResult::handled(suppress_text)
        };
    }
    if vim_visual_character_change_key(key, modifiers) {
        let repeat_count = vim_visual_character_repeat_count(buffer, anchor, cursor);
        let changed = vim_delete_visual_character(buffer, anchor, cursor, unnamed_register);
        *pending = None;
        return if changed {
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::SubstituteForwardChars,
                repeat_count,
                suppress_text,
            )
        } else {
            VimKeyResult::handled(suppress_text)
        };
    }
    if vim_visual_character_replace_key(key, modifiers) {
        *pending = Some(EditorVimPendingKey::VisualCharacterReplace { anchor, cursor });
        return VimKeyResult::handled(suppress_text);
    }
    if let Some(reverse) = vim_visual_character_char_find_repeat_key(key, modifiers) {
        if let Some(last) = *last_char_find {
            let motion = if reverse {
                last.motion.reversed()
            } else {
                last.motion
            };
            if let Some(target) = vim_visual_character_char_find_target(
                buffer,
                cursor,
                count.unwrap_or(1),
                motion,
                last.target,
            ) {
                vim_set_visual_character_selection(buffer, anchor, target);
                *pending = Some(EditorVimPendingKey::VisualCharacter {
                    anchor,
                    cursor: target,
                });
                return VimKeyResult::handled(suppress_text);
            }
        }

        vim_set_visual_character_selection(buffer, anchor, cursor);
        *pending = Some(EditorVimPendingKey::VisualCharacter { anchor, cursor });
        return VimKeyResult::handled(suppress_text);
    }

    if let Some(target) =
        vim_visual_character_motion_target(buffer, cursor, count.unwrap_or(1), key, modifiers)
    {
        vim_set_visual_character_selection(buffer, anchor, target);
        *pending = Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor: target,
        });
        return VimKeyResult::handled(suppress_text);
    }

    vim_restore_visual_character_pending(pending, anchor, cursor, count);
    if suppress_text.is_some() {
        VimKeyResult::handled(suppress_text)
    } else {
        VimKeyResult::ignored()
    }
}

pub(super) fn handle_vim_visual_character_go_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    indent_unit: &str,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    match key {
        Key::Num8 if modifiers.shift => {
            buffer.set_single_cursor(cursor.min(buffer.len_chars()));
            vim_search_word_under_cursor(buffer, count.unwrap_or(1), true, false);
            let target = vim_visual_character_clamped_cursor(buffer, buffer.cursor());
            vim_set_visual_character_selection(buffer, anchor, target);
            *pending = Some(EditorVimPendingKey::VisualCharacter {
                anchor,
                cursor: target,
            });
            VimKeyResult::handled(suppress_text)
        }
        Key::Num3 if modifiers.shift => {
            buffer.set_single_cursor(cursor.min(buffer.len_chars()));
            vim_search_word_under_cursor(buffer, count.unwrap_or(1), false, false);
            let target = vim_visual_character_clamped_cursor(buffer, buffer.cursor());
            vim_set_visual_character_selection(buffer, anchor, target);
            *pending = Some(EditorVimPendingKey::VisualCharacter {
                anchor,
                cursor: target,
            });
            VimKeyResult::handled(suppress_text)
        }
        Key::J if modifiers.shift => {
            vim_restore_visual_character_pending(pending, anchor, cursor, count);
            if suppress_text.is_some() {
                VimKeyResult::handled(suppress_text)
            } else {
                VimKeyResult::ignored()
            }
        }
        _ => handle_vim_visual_character_key_event(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            anchor,
            cursor,
            count,
            indent_unit,
            suppress_text,
        ),
    }
}

pub(super) fn handle_vim_visual_character_replace_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_change: &mut Option<EditorVimLastChange>,
    anchor: usize,
    cursor: usize,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    if vim_escape_key(key, modifiers) {
        *pending = None;
        buffer.set_single_cursor(cursor);
        return VimKeyResult::handled(suppress_text);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        *pending = Some(EditorVimPendingKey::VisualCharacterReplace { anchor, cursor });
        return VimKeyResult::ignored();
    }
    if let Some(replacement) = suppress_text {
        let repeat_count = vim_visual_character_repeat_count(buffer, anchor, cursor);
        let changed = vim_replace_visual_character(buffer, anchor, cursor, replacement);
        *pending = None;
        return vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::ReplaceForwardChars(replacement),
            repeat_count,
            suppress_text,
        );
    }

    *pending = Some(EditorVimPendingKey::VisualCharacterReplace { anchor, cursor });
    VimKeyResult::ignored()
}

pub(super) fn handle_vim_visual_character_char_find_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    motion: EditorVimCharFindMotion,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    if vim_escape_key(key, modifiers) {
        *pending = None;
        buffer.set_single_cursor(cursor);
        return VimKeyResult::handled(suppress_text);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        *pending = Some(EditorVimPendingKey::VisualCharacterCharFind {
            anchor,
            cursor,
            count,
            motion,
        });
        return VimKeyResult::ignored();
    }
    if let Some(target_char) = suppress_text {
        if let Some(target) = vim_visual_character_char_find_target(
            buffer,
            cursor,
            count.unwrap_or(1),
            motion,
            target_char,
        ) {
            *last_char_find = Some(EditorVimCharFind {
                motion,
                target: target_char,
            });
            vim_set_visual_character_selection(buffer, anchor, target);
            *pending = Some(EditorVimPendingKey::VisualCharacter {
                anchor,
                cursor: target,
            });
        } else {
            vim_set_visual_character_selection(buffer, anchor, cursor);
            *pending = Some(EditorVimPendingKey::VisualCharacter { anchor, cursor });
        }
        return VimKeyResult::handled(suppress_text);
    }

    *pending = Some(EditorVimPendingKey::VisualCharacterCharFind {
        anchor,
        cursor,
        count,
        motion,
    });
    VimKeyResult::ignored()
}

pub(super) fn handle_vim_visual_character_text_object_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    scope: EditorVimTextObjectScope,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    if vim_escape_key(key, modifiers) {
        *pending = None;
        buffer.set_single_cursor(cursor);
        return VimKeyResult::handled(suppress_text);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        *pending = Some(EditorVimPendingKey::VisualCharacterTextObject {
            anchor,
            cursor,
            count,
            scope,
        });
        return VimKeyResult::ignored();
    }
    if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
        let original_cursor = buffer.cursor();
        buffer.set_single_cursor(cursor);
        let range = vim_text_object_range(buffer, count.unwrap_or(1), scope, kind);
        buffer.set_single_cursor(original_cursor);

        if let Some(range) = range
            && range.start < range.end
        {
            let object_cursor = range.end.saturating_sub(1);
            vim_set_visual_character_selection(buffer, range.start, object_cursor);
            *pending = Some(EditorVimPendingKey::VisualCharacter {
                anchor: range.start,
                cursor: object_cursor,
            });
            return VimKeyResult::handled(suppress_text);
        }

        vim_set_visual_character_selection(buffer, anchor, cursor);
        *pending = Some(EditorVimPendingKey::VisualCharacter { anchor, cursor });
        return VimKeyResult::handled(suppress_text);
    }

    *pending = Some(EditorVimPendingKey::VisualCharacterTextObject {
        anchor,
        cursor,
        count,
        scope,
    });
    if suppress_text.is_some() {
        VimKeyResult::handled(suppress_text)
    } else {
        VimKeyResult::ignored()
    }
}

pub(super) fn handle_vim_visual_character_register_prefix_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    if vim_escape_key(key, modifiers) {
        *pending = None;
        buffer.set_single_cursor(cursor);
        return VimKeyResult::handled(suppress_text);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        *pending = Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
            anchor,
            cursor,
            count,
        });
        return VimKeyResult::ignored();
    }
    if let Some(register) = vim_named_register_for_key(key, modifiers) {
        *pending = Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor,
            cursor,
            count,
            register,
        });
        return VimKeyResult::handled(suppress_text);
    }

    *pending = Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
        anchor,
        cursor,
        count,
    });
    if suppress_text.is_some() {
        VimKeyResult::handled(suppress_text)
    } else {
        VimKeyResult::ignored()
    }
}

pub(super) fn handle_vim_visual_character_register_command_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    register: EditorVimNamedRegister,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let anchor = vim_visual_character_clamped_cursor(buffer, anchor);
    let cursor = vim_visual_character_clamped_cursor(buffer, cursor);
    if vim_escape_key(key, modifiers) {
        *pending = None;
        buffer.set_single_cursor(cursor);
        return VimKeyResult::handled(suppress_text);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        *pending = Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor,
            cursor,
            count,
            register,
        });
        return VimKeyResult::ignored();
    }
    if vim_visual_character_yank_key(key, modifiers) {
        vim_yank_visual_character_into_named_register(
            buffer,
            anchor,
            cursor,
            unnamed_register,
            register,
        );
        *pending = None;
        return VimKeyResult::handled(suppress_text);
    }
    if vim_visual_character_delete_key(key, modifiers) {
        let changed = vim_delete_visual_character_into_named_register(
            buffer,
            anchor,
            cursor,
            unnamed_register,
            register,
        );
        *pending = None;
        return if changed {
            VimKeyResult::changed(suppress_text)
        } else {
            VimKeyResult::handled(suppress_text)
        };
    }
    if vim_visual_character_change_key(key, modifiers) {
        let repeat_count = vim_visual_character_repeat_count(buffer, anchor, cursor);
        let changed = vim_delete_visual_character_into_named_register(
            buffer,
            anchor,
            cursor,
            unnamed_register,
            register,
        );
        *pending = None;
        return if changed {
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::SubstituteForwardCharsIntoRegister(register),
                repeat_count,
                suppress_text,
            )
        } else {
            VimKeyResult::handled(suppress_text)
        };
    }

    *pending = Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
        anchor,
        cursor,
        count,
        register,
    });
    if suppress_text.is_some() {
        VimKeyResult::handled(suppress_text)
    } else {
        VimKeyResult::ignored()
    }
}

pub(super) fn vim_visual_pending_after_key(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
) -> Option<Option<EditorVimPendingKey>> {
    let (anchor, cursor, count) = match pending {
        Some(EditorVimPendingKey::VisualCharacter { anchor, cursor }) => (anchor, cursor, None),
        Some(EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count,
        }) => (anchor, cursor, Some(count)),
        Some(EditorVimPendingKey::VisualCharacterGo {
            anchor,
            cursor,
            count: _,
        }) => {
            if vim_escape_key(key, modifiers) {
                return Some(None);
            }
            if modifiers.command || modifiers.alt || modifiers.ctrl {
                return None;
            }
            if matches!(
                (key, modifiers.shift),
                (Key::Num8, true) | (Key::Num3, true)
            ) {
                return Some(Some(EditorVimPendingKey::VisualCharacter {
                    anchor,
                    cursor,
                }));
            }
            return printable_key_char.is_some().then_some(Some(
                EditorVimPendingKey::VisualCharacter { anchor, cursor },
            ));
        }
        Some(EditorVimPendingKey::VisualCharacterTextObject {
            anchor,
            cursor,
            count,
            scope,
        }) => {
            if vim_escape_key(key, modifiers) {
                return Some(None);
            }
            if modifiers.command || modifiers.alt || modifiers.ctrl {
                return None;
            }
            if vim_text_object_kind_for_key(key, modifiers).is_some() {
                return Some(Some(EditorVimPendingKey::VisualCharacter {
                    anchor,
                    cursor,
                }));
            }
            return printable_key_char.is_some().then_some(Some(
                EditorVimPendingKey::VisualCharacterTextObject {
                    anchor,
                    cursor,
                    count,
                    scope,
                },
            ));
        }
        Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
            anchor,
            cursor,
            count,
        }) => {
            if vim_escape_key(key, modifiers) {
                return Some(None);
            }
            if modifiers.command || modifiers.alt || modifiers.ctrl {
                return None;
            }
            if let Some(register) = vim_named_register_for_key(key, modifiers) {
                return Some(Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
                    anchor,
                    cursor,
                    count,
                    register,
                }));
            }
            return printable_key_char.is_some().then_some(pending);
        }
        Some(EditorVimPendingKey::VisualCharacterRegisterCommand {
            anchor,
            cursor,
            count,
            register,
        }) => {
            if vim_escape_key(key, modifiers)
                || vim_visual_character_yank_key(key, modifiers)
                || vim_visual_character_delete_key(key, modifiers)
                || vim_visual_character_change_key(key, modifiers)
            {
                return Some(None);
            }
            if modifiers.command || modifiers.alt || modifiers.ctrl {
                return None;
            }
            return printable_key_char.is_some().then_some(Some(
                EditorVimPendingKey::VisualCharacterRegisterCommand {
                    anchor,
                    cursor,
                    count,
                    register,
                },
            ));
        }
        Some(EditorVimPendingKey::VisualCharacterReplace { .. }) => {
            if vim_escape_key(key, modifiers) {
                return Some(None);
            }
            if modifiers.command || modifiers.alt || modifiers.ctrl {
                return None;
            }
            return printable_key_char.is_some().then_some(None);
        }
        Some(EditorVimPendingKey::VisualCharacterCharFind { anchor, cursor, .. }) => {
            if vim_escape_key(key, modifiers) {
                return Some(None);
            }
            if modifiers.command || modifiers.alt || modifiers.ctrl {
                return None;
            }
            return printable_key_char.is_some().then_some(Some(
                EditorVimPendingKey::VisualCharacter { anchor, cursor },
            ));
        }
        _ => return None,
    };
    if vim_escape_key(key, modifiers)
        || vim_visual_character_toggle_key(key, modifiers)
        || vim_visual_character_yank_key(key, modifiers)
        || vim_visual_character_join_key(key, modifiers)
        || vim_visual_character_indent_key(key, modifiers)
        || vim_visual_character_outdent_key(key, modifiers)
        || vim_visual_character_case_conversion(key, modifiers).is_some()
        || vim_visual_character_delete_key(key, modifiers)
        || vim_visual_character_change_key(key, modifiers)
    {
        return Some(None);
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if key == Key::Quote && modifiers.shift {
        return Some(Some(EditorVimPendingKey::VisualCharacterRegisterPrefix {
            anchor,
            cursor,
            count,
        }));
    }
    if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
        return Some(Some(EditorVimPendingKey::VisualCharacterTextObject {
            anchor,
            cursor,
            count,
            scope,
        }));
    }
    if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
        return Some(Some(EditorVimPendingKey::VisualCharacterCharFind {
            anchor,
            cursor,
            count,
            motion,
        }));
    }
    if vim_visual_character_replace_key(key, modifiers) {
        return Some(Some(EditorVimPendingKey::VisualCharacterReplace {
            anchor,
            cursor,
        }));
    }
    if vim_visual_character_char_find_repeat_key(key, modifiers).is_some() {
        return Some(Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor,
        }));
    }
    if vim_visual_character_swap_key(key, modifiers) {
        return Some(Some(EditorVimPendingKey::VisualCharacter {
            anchor: cursor,
            cursor: anchor,
        }));
    }
    if key == Key::G && !modifiers.shift {
        return Some(Some(EditorVimPendingKey::VisualCharacterGo {
            anchor,
            cursor,
            count,
        }));
    }
    if count.is_none()
        && let Some(digit) = vim_count_digit(key, modifiers, false)
    {
        return Some(Some(EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count: digit,
        }));
    }
    if let Some(count) = count
        && let Some(digit) = vim_count_digit(key, modifiers, true)
    {
        return Some(Some(EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count: vim_push_count_digit(count, digit),
        }));
    }
    if vim_visual_character_motion_key(key, modifiers) {
        return Some(Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor,
        }));
    }
    printable_key_char.is_some().then_some(pending)
}

pub(super) fn vim_restore_visual_character_pending(
    pending: &mut Option<EditorVimPendingKey>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
) {
    *pending = Some(if let Some(count) = count {
        EditorVimPendingKey::VisualCharacterCount {
            anchor,
            cursor,
            count,
        }
    } else {
        EditorVimPendingKey::VisualCharacter { anchor, cursor }
    });
}

pub(super) fn vim_cancel_pending_visual_character(
    buffer: &mut TextBuffer,
    pending: Option<EditorVimPendingKey>,
) {
    if let Some(
        EditorVimPendingKey::VisualCharacter { cursor, .. }
        | EditorVimPendingKey::VisualCharacterCount { cursor, .. }
        | EditorVimPendingKey::VisualCharacterGo { cursor, .. }
        | EditorVimPendingKey::VisualCharacterReplace { cursor, .. }
        | EditorVimPendingKey::VisualCharacterCharFind { cursor, .. }
        | EditorVimPendingKey::VisualCharacterTextObject { cursor, .. }
        | EditorVimPendingKey::VisualCharacterRegisterPrefix { cursor, .. }
        | EditorVimPendingKey::VisualCharacterRegisterCommand { cursor, .. },
    ) = pending
    {
        buffer.set_single_cursor(vim_visual_character_clamped_cursor(buffer, cursor));
    }
}

pub(super) fn vim_visual_character_toggle_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::V && !modifiers.shift && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_yank_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::Y && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_join_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::J && modifiers.shift && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_indent_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::Period && modifiers.shift && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_outdent_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::Comma && modifiers.shift && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_swap_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::O && !modifiers.shift && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_case_conversion(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimCaseConversion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::Backtick, true) => Some(EditorVimCaseConversion::Toggle),
        (Key::U, false) => Some(EditorVimCaseConversion::Lower),
        (Key::U, true) => Some(EditorVimCaseConversion::Upper),
        _ => None,
    }
}

pub(super) fn vim_visual_character_delete_key(key: Key, modifiers: Modifiers) -> bool {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return false;
    }
    matches!(
        (key, modifiers.shift),
        (Key::D, false) | (Key::D, true) | (Key::X, false) | (Key::X, true)
    )
}

pub(super) fn vim_visual_character_change_key(key: Key, modifiers: Modifiers) -> bool {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return false;
    }
    matches!(
        (key, modifiers.shift),
        (Key::C, false) | (Key::C, true) | (Key::S, false) | (Key::S, true)
    )
}

pub(super) fn vim_visual_character_replace_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::R && !modifiers.shift && !modifiers.command && !modifiers.alt && !modifiers.ctrl
}

pub(super) fn vim_visual_character_char_find_repeat_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<bool> {
    if modifiers.command || modifiers.alt || modifiers.ctrl || modifiers.shift {
        return None;
    }
    match key {
        Key::Semicolon => Some(false),
        Key::Comma => Some(true),
        _ => None,
    }
}

pub(super) fn vim_visual_character_clamped_cursor(buffer: &TextBuffer, cursor: usize) -> usize {
    let len = buffer.len_chars();
    if len == 0 {
        return 0;
    }

    let cursor = cursor.min(len);
    if cursor == len {
        return len - 1;
    }

    let position = buffer.char_position(cursor);
    let line_start = buffer.line_column_to_char(position.line, 0);
    let line_content_end = buffer.line_content_end_char(position.line);
    if cursor == line_content_end && cursor > line_start {
        cursor - 1
    } else {
        cursor
    }
}

pub(super) fn vim_set_visual_character_selection(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
) {
    let len = buffer.len_chars();
    if len == 0 {
        buffer.set_single_cursor(0);
        return;
    }

    let anchor = anchor.min(len);
    let cursor = cursor.min(len);
    if cursor >= anchor {
        buffer.set_selection(anchor, cursor.saturating_add(1).min(len));
    } else {
        buffer.set_selection(anchor.saturating_add(1).min(len), cursor);
    }
}

pub(super) fn vim_visual_character_range(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> Option<Range<usize>> {
    let len = buffer.len_chars();
    if len == 0 {
        return None;
    }
    let anchor = anchor.min(len);
    let cursor = cursor.min(len);
    let start = anchor.min(cursor);
    let end = anchor.max(cursor).saturating_add(1).min(len);
    (start < end).then_some(start..end)
}

pub(super) fn vim_visual_character_repeat_count(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> usize {
    vim_visual_character_range(buffer, anchor, cursor)
        .map(|range| {
            range
                .end
                .saturating_sub(range.start)
                .clamp(1, VIM_MAX_COUNT)
        })
        .unwrap_or(1)
}

pub(super) fn vim_visual_character_line_span(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> Option<(Range<usize>, usize, usize)> {
    let selection = vim_visual_character_range(buffer, anchor, cursor)?;
    let last_selected_char = selection.end.checked_sub(1)?;
    let start_line = buffer.char_position(selection.start).line;
    let end_line = buffer.char_position(last_selected_char).line;
    let start = buffer.line_column_to_char(start_line, 0);
    let end = if end_line + 1 < buffer.len_lines() {
        buffer.line_column_to_char(end_line + 1, 0)
    } else {
        buffer.len_chars()
    };
    let line_count = end_line.saturating_sub(start_line).saturating_add(1);
    (start < end).then_some((start..end, selection.start, line_count))
}

pub(super) fn vim_visual_character_line_repeat_count(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> usize {
    vim_visual_character_line_span(buffer, anchor, cursor)
        .map(|(_, _, line_count)| line_count.clamp(1, VIM_MAX_COUNT))
        .unwrap_or(1)
}

pub(super) fn vim_visual_character_join_repeat_count(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> usize {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        return 1;
    };
    let Some(last_selected_char) = range.end.checked_sub(1) else {
        return 1;
    };

    let start_line = buffer.char_position(range.start).line;
    let end_line = buffer.char_position(last_selected_char).line;
    end_line
        .saturating_sub(start_line)
        .max(1)
        .clamp(1, VIM_MAX_COUNT)
}

pub(super) fn vim_join_visual_character_lines(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let cursor = range.start;
    buffer.set_selection(range.start, range.end);
    let changed = buffer.join_lines();
    buffer.set_single_cursor(cursor.min(buffer.len_chars()));
    changed
}

pub(super) fn vim_indent_visual_character_lines(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    indent_unit: &str,
) -> bool {
    let Some((range, selection_start, _)) = vim_visual_character_line_span(buffer, anchor, cursor)
    else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let position = buffer.char_position(selection_start);
    if indent_unit.is_empty() {
        buffer.set_single_cursor(selection_start.min(buffer.len_chars()));
        return false;
    }

    buffer.set_selection(range.start, range.end);
    let changed = buffer.indent_lines(indent_unit);
    if changed {
        let column = position.column.saturating_add(indent_unit.chars().count());
        buffer.set_single_cursor(buffer.line_column_to_char(position.line, column));
    } else {
        buffer.set_single_cursor(selection_start.min(buffer.len_chars()));
    }
    changed
}

pub(super) fn vim_outdent_visual_character_lines(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    indent_unit: &str,
) -> bool {
    let Some((range, selection_start, _)) = vim_visual_character_line_span(buffer, anchor, cursor)
    else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let position = buffer.char_position(selection_start);
    let remove_len = vim_line_outdent_len(buffer, position.line, indent_unit);

    buffer.set_selection(range.start, range.end);
    let changed = buffer.outdent_lines(indent_unit);
    if changed {
        let column = position.column.saturating_sub(remove_len);
        buffer.set_single_cursor(buffer.line_column_to_char(position.line, column));
    } else {
        buffer.set_single_cursor(selection_start.min(buffer.len_chars()));
    }
    changed
}

pub(super) fn vim_yank_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_yank_visual_character_into_registers(buffer, anchor, cursor, unnamed_register, None)
}

pub(super) fn vim_yank_visual_character_into_named_register(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_yank_visual_character_into_registers(
        buffer,
        anchor,
        cursor,
        unnamed_register,
        Some(named_register),
    )
}

pub(super) fn vim_yank_visual_character_into_registers(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let yanked = vim_yank_range_into_register(
        buffer,
        range.clone(),
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        named_register,
    );
    buffer.set_single_cursor(range.start);
    yanked
}

pub(super) fn vim_delete_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_delete_visual_character_into_registers(buffer, anchor, cursor, unnamed_register, None)
}

pub(super) fn vim_delete_visual_character_into_named_register(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_delete_visual_character_into_registers(
        buffer,
        anchor,
        cursor,
        unnamed_register,
        Some(named_register),
    )
}

pub(super) fn vim_delete_visual_character_into_registers(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    vim_delete_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        named_register,
    )
}

pub(super) fn vim_convert_case_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    conversion: EditorVimCaseConversion,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    vim_convert_case_range(buffer, range.clone(), range.start, conversion)
}

pub(super) fn vim_replace_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    replacement: char,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let replaced_len = range.end.saturating_sub(range.start);
    if replaced_len == 0 {
        buffer.set_single_cursor(range.start);
        return false;
    }

    let inserted = std::iter::repeat_n(replacement, replaced_len).collect::<String>();
    let edit = TextEdit { range, inserted };
    buffer.apply_edits_with_inserted_selection(vec![edit.clone()], &edit, 0..0)
}

pub(super) fn vim_visual_character_char_find_target(
    buffer: &mut TextBuffer,
    cursor: usize,
    count: usize,
    motion: EditorVimCharFindMotion,
    target: char,
) -> Option<usize> {
    let original_cursor = buffer.cursor();
    buffer.set_single_cursor(cursor.min(buffer.len_chars()));
    let found = vim_apply_char_find(buffer, count, motion, target);
    let target_cursor = vim_visual_character_clamped_cursor(buffer, buffer.cursor());
    buffer.set_single_cursor(original_cursor.min(buffer.len_chars()));
    found.then_some(target_cursor)
}

pub(super) fn vim_visual_character_motion_key(key: Key, modifiers: Modifiers) -> bool {
    if vim_operator_motion_for_key(key, modifiers).is_some() {
        return true;
    }
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return false;
    }
    if matches!(key, Key::Enter) {
        return no_text_modifiers(modifiers);
    }
    matches!(
        (key, modifiers.shift),
        (Key::Equals, true)
            | (Key::J, false)
            | (Key::K, false)
            | (Key::Minus, false)
            | (Key::Minus, true)
            | (Key::N, false)
            | (Key::N, true)
            | (Key::Num3, true)
            | (Key::Num8, true)
    )
}

pub(super) fn vim_visual_character_motion_target(
    buffer: &mut TextBuffer,
    cursor: usize,
    count: usize,
    key: Key,
    modifiers: Modifiers,
) -> Option<usize> {
    if !vim_visual_character_motion_key(key, modifiers) {
        return None;
    }
    let count = count.clamp(1, VIM_MAX_COUNT);
    buffer.set_single_cursor(cursor.min(buffer.len_chars()));
    match key {
        Key::H if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_left();
            }
        }
        Key::Backspace if no_text_modifiers(modifiers) => {
            vim_move_space_backward(buffer, count);
        }
        Key::Enter if no_text_modifiers(modifiers) => {
            vim_move_next_line_first_non_whitespace(buffer, count);
        }
        Key::Equals if modifiers.shift => {
            vim_move_next_line_first_non_whitespace(buffer, count);
        }
        Key::Minus if !modifiers.shift => {
            vim_move_previous_line_first_non_whitespace(buffer, count);
        }
        Key::Minus if modifiers.shift => {
            vim_move_counted_line_first_non_whitespace(buffer, count);
        }
        Key::J if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_down();
            }
        }
        Key::K if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_up();
            }
        }
        Key::L if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_right();
            }
        }
        Key::Space if no_text_modifiers(modifiers) => {
            vim_move_space_forward(buffer, count);
        }
        Key::W if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_word_right();
            }
        }
        Key::W if modifiers.shift => {
            for _ in 0..count {
                buffer.move_big_word_right();
            }
        }
        Key::E if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_word_end();
            }
        }
        Key::E if modifiers.shift => {
            for _ in 0..count {
                buffer.move_big_word_end();
            }
        }
        Key::B if !modifiers.shift => {
            for _ in 0..count {
                buffer.move_word_left();
            }
        }
        Key::B if modifiers.shift => {
            for _ in 0..count {
                buffer.move_big_word_left();
            }
        }
        Key::Num0 if !modifiers.shift => {
            buffer.move_line_column_start();
        }
        key if vim_line_column_motion_key(key, modifiers) => {
            vim_move_to_line_column(buffer, count);
        }
        Key::Home if no_text_modifiers(modifiers) => {
            buffer.move_line_column_start();
        }
        Key::Num6 if modifiers.shift => {
            buffer.move_line_first_non_whitespace();
        }
        Key::Num4 if modifiers.shift => {
            vim_move_to_visual_line_end(buffer, count);
        }
        Key::End if no_text_modifiers(modifiers) => {
            vim_move_to_visual_line_end(buffer, count);
        }
        Key::Num5 if modifiers.shift => {
            vim_move_to_matching_bracket(buffer);
        }
        Key::CloseBracket if modifiers.shift => {
            vim_move_next_paragraph(buffer, count);
        }
        Key::OpenBracket if modifiers.shift => {
            vim_move_previous_paragraph(buffer, count);
        }
        Key::Num8 if modifiers.shift => {
            vim_search_word_under_cursor(buffer, count, true, true);
        }
        Key::Num3 if modifiers.shift => {
            vim_search_word_under_cursor(buffer, count, false, true);
        }
        Key::N if !modifiers.shift => {
            vim_repeat_last_search(buffer, count, false);
        }
        Key::N if modifiers.shift => {
            vim_repeat_last_search(buffer, count, true);
        }
        _ => return None,
    }
    Some(vim_visual_character_clamped_cursor(buffer, buffer.cursor()))
}

pub(super) fn vim_move_to_visual_line_end(buffer: &mut TextBuffer, count: usize) {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    let line_start = buffer.line_column_to_char(line, 0);
    let content_end = buffer.line_content_end_char(line);
    buffer.set_single_cursor(content_end.saturating_sub(1).max(line_start));
}
