use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::commands::{
    vim_apply_last_change, vim_delete_lines_into_register, vim_put_register_after,
    vim_put_register_before, vim_yank_lines,
};
use super::motion::{
    vim_apply_char_find, vim_delete_backward_chars, vim_delete_forward_chars, vim_join_lines,
    vim_move_counted_line_first_non_whitespace, vim_move_next_line_first_non_whitespace,
    vim_move_next_paragraph, vim_move_previous_line_first_non_whitespace,
    vim_move_previous_paragraph, vim_move_space_backward, vim_move_space_forward,
    vim_move_to_line_column, vim_move_to_line_end, vim_move_to_matching_bracket,
    vim_open_line_above, vim_open_line_below, vim_toggle_case_forward_chars,
};
use super::operator::vim_delete_to_line_end;
use super::search::{vim_clear_search_input, vim_repeat_last_search, vim_search_word_under_cursor};
use super::visual::{vim_set_visual_character_selection, vim_visual_character_clamped_cursor};
use super::{
    EditorVimCharFind, EditorVimLastChange, EditorVimMode, EditorVimPendingKey, EditorVimRegister,
    EditorVimRepeatAction, VimKeyResult, no_text_modifiers, vim_count_digit, vim_go_to_line,
    vim_line_column_motion_key, vim_record_insert_change, vim_repeatable_change_result,
    vim_search_direction_for_key,
};

pub(super) fn handle_vim_direct_normal_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    indent_unit: &str,
    count: Option<usize>,
    count_value: usize,
    suppress_text: Option<char>,
) -> VimKeyResult {
    match key {
        Key::Escape => VimKeyResult::handled(None),
        key if vim_count_digit(key, modifiers, false).is_some() => {
            let count = vim_count_digit(key, modifiers, false).unwrap_or(1);
            *pending = Some(EditorVimPendingKey::Count(count));
            VimKeyResult::handled(suppress_text)
        }
        Key::Quote if modifiers.shift => {
            *pending = Some(EditorVimPendingKey::RegisterPrefix(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::D if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::DeleteLine(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::C if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::ChangeLine(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::Y if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::YankLine(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::Y if modifiers.shift => {
            vim_yank_lines(buffer, count_value, unnamed_register);
            VimKeyResult::handled(suppress_text)
        }
        Key::V if !modifiers.shift => {
            let cursor = vim_visual_character_clamped_cursor(buffer, buffer.cursor());
            vim_set_visual_character_selection(buffer, cursor, cursor);
            *pending = Some(EditorVimPendingKey::VisualCharacter {
                anchor: cursor,
                cursor,
            });
            VimKeyResult::handled(suppress_text)
        }
        Key::F if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::FindCharForward(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::F if modifiers.shift => {
            *pending = Some(EditorVimPendingKey::FindCharBackward(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::T if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::TillCharForward(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::T if modifiers.shift => {
            *pending = Some(EditorVimPendingKey::TillCharBackward(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::Semicolon if !modifiers.shift => {
            if let Some(last) = *last_char_find {
                vim_apply_char_find(buffer, count_value, last.motion, last.target);
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::Comma if !modifiers.shift => {
            if let Some(last) = *last_char_find {
                vim_apply_char_find(buffer, count_value, last.motion.reversed(), last.target);
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::R if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::ReplaceChar(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::Period if !modifiers.shift => {
            if let Some(change) = last_change.clone() {
                if vim_apply_last_change(buffer, change, count, mode, unnamed_register, indent_unit)
                {
                    VimKeyResult::changed(suppress_text)
                } else {
                    VimKeyResult::handled(suppress_text)
                }
            } else {
                VimKeyResult::handled(suppress_text)
            }
        }
        Key::Period if modifiers.shift => {
            *pending = Some(EditorVimPendingKey::IndentLine(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::Comma if modifiers.shift => {
            *pending = Some(EditorVimPendingKey::OutdentLine(count_value));
            VimKeyResult::handled(suppress_text)
        }
        Key::P if !modifiers.shift => vim_repeatable_change_result(
            vim_put_register_after(buffer, unnamed_register.as_ref(), count_value),
            last_change,
            EditorVimRepeatAction::PutAfter,
            count_value,
            suppress_text,
        ),
        Key::P if modifiers.shift => vim_repeatable_change_result(
            vim_put_register_before(buffer, unnamed_register.as_ref(), count_value),
            last_change,
            EditorVimRepeatAction::PutBefore,
            count_value,
            suppress_text,
        ),
        Key::Backtick if modifiers.shift => vim_repeatable_change_result(
            vim_toggle_case_forward_chars(buffer, count_value),
            last_change,
            EditorVimRepeatAction::ToggleCaseForwardChars,
            count_value,
            suppress_text,
        ),
        Key::D if modifiers.shift => vim_repeatable_change_result(
            vim_delete_to_line_end(buffer, count_value),
            last_change,
            EditorVimRepeatAction::DeleteToLineEnd,
            count_value,
            suppress_text,
        ),
        Key::C if modifiers.shift => {
            let changed = vim_delete_to_line_end(buffer, count_value);
            *pending = None;
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::ChangeToLineEnd,
                count_value,
                suppress_text,
            )
        }
        Key::S if !modifiers.shift => {
            let changed = vim_delete_forward_chars(buffer, count_value);
            *pending = None;
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::SubstituteForwardChars,
                count_value,
                suppress_text,
            )
        }
        Key::S if modifiers.shift => {
            let changed = vim_delete_lines_into_register(buffer, count_value, unnamed_register);
            *pending = None;
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::ChangeLines,
                count_value,
                suppress_text,
            )
        }
        Key::G if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::Go(count));
            VimKeyResult::handled(suppress_text)
        }
        key if vim_search_direction_for_key(key, modifiers).is_some() => {
            let forward = vim_search_direction_for_key(key, modifiers).unwrap_or(true);
            vim_clear_search_input();
            *pending = Some(EditorVimPendingKey::SearchInput {
                count: count_value,
                forward,
            });
            VimKeyResult::handled(suppress_text)
        }
        Key::M if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::SetMark);
            VimKeyResult::handled(suppress_text)
        }
        Key::Quote if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::JumpMark { linewise: true });
            VimKeyResult::handled(suppress_text)
        }
        Key::Backtick if !modifiers.shift => {
            *pending = Some(EditorVimPendingKey::JumpMark { linewise: false });
            VimKeyResult::handled(suppress_text)
        }
        Key::H if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_left();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::Backspace if no_text_modifiers(modifiers) => {
            vim_move_space_backward(buffer, count_value);
            VimKeyResult::handled(None)
        }
        Key::Enter if no_text_modifiers(modifiers) => {
            vim_move_next_line_first_non_whitespace(buffer, count_value);
            VimKeyResult::handled(None)
        }
        Key::Equals if modifiers.shift => {
            vim_move_next_line_first_non_whitespace(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::Minus if !modifiers.shift => {
            vim_move_previous_line_first_non_whitespace(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::Minus if modifiers.shift => {
            vim_move_counted_line_first_non_whitespace(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::J if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_down();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::J if modifiers.shift => vim_repeatable_change_result(
            vim_join_lines(buffer, count_value),
            last_change,
            EditorVimRepeatAction::JoinLines,
            count_value,
            suppress_text,
        ),
        Key::K if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_up();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::L if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_right();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::Space if no_text_modifiers(modifiers) => {
            vim_move_space_forward(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::W if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_word_right();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::W if modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_big_word_right();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::E if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_word_end();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::E if modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_big_word_end();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::B if !modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_word_left();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::B if modifiers.shift => {
            for _ in 0..count_value {
                buffer.move_big_word_left();
            }
            VimKeyResult::handled(suppress_text)
        }
        Key::Num0 if !modifiers.shift => {
            buffer.move_line_column_start();
            VimKeyResult::handled(suppress_text)
        }
        key if vim_line_column_motion_key(key, modifiers) => {
            vim_move_to_line_column(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::Home if no_text_modifiers(modifiers) => {
            buffer.move_line_column_start();
            VimKeyResult::handled(suppress_text)
        }
        Key::Num6 if modifiers.shift => {
            buffer.move_line_first_non_whitespace();
            VimKeyResult::handled(suppress_text)
        }
        Key::Num4 if modifiers.shift => {
            vim_move_to_line_end(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::End if no_text_modifiers(modifiers) => {
            vim_move_to_line_end(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::Num5 if modifiers.shift => {
            vim_move_to_matching_bracket(buffer);
            VimKeyResult::handled(suppress_text)
        }
        Key::CloseBracket if modifiers.shift => {
            vim_move_next_paragraph(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::OpenBracket if modifiers.shift => {
            vim_move_previous_paragraph(buffer, count_value);
            VimKeyResult::handled(suppress_text)
        }
        Key::Num8 if modifiers.shift => {
            vim_search_word_under_cursor(buffer, count_value, true, true);
            VimKeyResult::handled(suppress_text)
        }
        Key::Num3 if modifiers.shift => {
            vim_search_word_under_cursor(buffer, count_value, false, true);
            VimKeyResult::handled(suppress_text)
        }
        Key::N if !modifiers.shift => {
            vim_repeat_last_search(buffer, count_value, false);
            VimKeyResult::handled(suppress_text)
        }
        Key::N if modifiers.shift => {
            vim_repeat_last_search(buffer, count_value, true);
            VimKeyResult::handled(suppress_text)
        }
        Key::I => {
            if modifiers.shift {
                buffer.move_line_first_non_whitespace();
                vim_record_insert_change(
                    last_change,
                    EditorVimRepeatAction::InsertLineFirstNonWhitespace,
                );
            } else {
                vim_record_insert_change(last_change, EditorVimRepeatAction::InsertAtCursor);
            }
            *pending = None;
            *mode = EditorVimMode::Insert;
            VimKeyResult::handled(suppress_text)
        }
        Key::A => {
            if modifiers.shift {
                buffer.move_line_end();
                vim_record_insert_change(last_change, EditorVimRepeatAction::InsertLineEnd);
            } else {
                buffer.move_right();
                vim_record_insert_change(last_change, EditorVimRepeatAction::AppendAfterCursor);
            }
            *pending = None;
            *mode = EditorVimMode::Insert;
            VimKeyResult::handled(suppress_text)
        }
        Key::O => {
            if modifiers.shift {
                vim_open_line_above(buffer);
            } else {
                vim_open_line_below(buffer);
            }
            *pending = None;
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                true,
                last_change,
                if modifiers.shift {
                    EditorVimRepeatAction::OpenLineAbove
                } else {
                    EditorVimRepeatAction::OpenLineBelow
                },
                1,
                suppress_text,
            )
        }
        Key::X if !modifiers.shift => {
            let mut changed = false;
            for _ in 0..count_value {
                changed |= buffer.delete_forward();
            }
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::DeleteForwardChars,
                count_value,
                suppress_text,
            )
        }
        Key::X if modifiers.shift => vim_repeatable_change_result(
            vim_delete_backward_chars(buffer, count_value),
            last_change,
            EditorVimRepeatAction::DeleteBackwardChars,
            count_value,
            suppress_text,
        ),
        Key::U if !modifiers.shift => {
            let mut changed = false;
            for _ in 0..count_value {
                changed |= buffer.undo();
            }
            if changed {
                VimKeyResult::changed(suppress_text)
            } else {
                VimKeyResult::handled(suppress_text)
            }
        }
        Key::G if modifiers.shift => {
            match count {
                Some(line) => vim_go_to_line(buffer, line),
                None => {
                    let last_line = buffer.len_lines().saturating_sub(1);
                    let cursor = buffer.line_column_to_char(last_line, 0);
                    buffer.set_single_cursor(cursor);
                }
            }
            VimKeyResult::handled(suppress_text)
        }
        _ => VimKeyResult::ignored(),
    }
}
