use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::{
    EditorVimCharFindMotion, EditorVimOperatorMotion, VIM_MAX_COUNT, vim_apply_char_find,
    vim_line_column_motion_char, vim_line_first_non_whitespace_char, vim_matching_bracket_range,
    vim_move_previous_big_word_end, vim_next_paragraph_line, vim_operator_search_match_range,
    vim_operator_search_repeat_range, vim_operator_search_word_under_cursor_range,
    vim_previous_paragraph_line,
};

pub(super) fn vim_operator_motion_range(
    buffer: &mut TextBuffer,
    count: usize,
    motion: EditorVimOperatorMotion,
) -> Option<Range<usize>> {
    let start = buffer.cursor();
    match motion {
        EditorVimOperatorMotion::LineColumnStart => {
            let line_start = buffer.line_column_to_char(buffer.cursor_position().line, 0);
            return (line_start != start).then_some(line_start.min(start)..line_start.max(start));
        }
        EditorVimOperatorMotion::LineColumn => {
            let target = vim_line_column_motion_char(buffer, count);
            return (target != start).then_some(target.min(start)..target.max(start));
        }
        EditorVimOperatorMotion::LineFirstNonWhitespace => {
            let first_non_whitespace =
                vim_line_first_non_whitespace_char(buffer, buffer.cursor_position().line);
            return (first_non_whitespace != start)
                .then_some(first_non_whitespace.min(start)..first_non_whitespace.max(start));
        }
        EditorVimOperatorMotion::LineEnd => {
            let count = count.clamp(1, VIM_MAX_COUNT);
            let line = buffer
                .cursor_position()
                .line
                .saturating_add(count.saturating_sub(1))
                .min(buffer.len_lines().saturating_sub(1));
            let end = buffer.line_content_end_char(line);
            return (end > start).then_some(start..end);
        }
        EditorVimOperatorMotion::MatchingBracket => {
            return vim_matching_bracket_range(buffer);
        }
        EditorVimOperatorMotion::ParagraphForward => {
            let end = vim_paragraph_motion_char(buffer, count, true);
            return (end > start).then_some(start..end);
        }
        EditorVimOperatorMotion::ParagraphBackward => {
            let end = vim_paragraph_motion_char(buffer, count, false);
            return (end < start).then_some(end..start);
        }
        EditorVimOperatorMotion::CharFind { motion, target } => {
            return vim_operator_char_find_range(buffer, count, motion, target);
        }
        EditorVimOperatorMotion::SearchRepeat { reverse } => {
            return vim_operator_search_repeat_range(buffer, count, reverse);
        }
        EditorVimOperatorMotion::SearchMatch { reverse } => {
            return vim_operator_search_match_range(buffer, count, reverse);
        }
        EditorVimOperatorMotion::SearchWordUnderCursor {
            forward,
            whole_word,
        } => {
            return vim_operator_search_word_under_cursor_range(buffer, count, forward, whole_word);
        }
        _ => {}
    }

    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        match motion {
            EditorVimOperatorMotion::BigWordBackward => buffer.move_big_word_left(),
            EditorVimOperatorMotion::BigWordEnd => buffer.move_big_word_end(),
            EditorVimOperatorMotion::BigWordEndBackward => vim_move_previous_big_word_end(buffer),
            EditorVimOperatorMotion::BigWordForward => buffer.move_big_word_right(),
            EditorVimOperatorMotion::CharFind { .. } => {}
            EditorVimOperatorMotion::CharacterBackward => buffer.move_left(),
            EditorVimOperatorMotion::CharacterForward => buffer.move_right(),
            EditorVimOperatorMotion::SearchMatch { .. } => {}
            EditorVimOperatorMotion::SearchRepeat { .. } => {}
            EditorVimOperatorMotion::SearchWordUnderCursor { .. } => {}
            EditorVimOperatorMotion::WordBackward => buffer.move_word_left(),
            EditorVimOperatorMotion::WordEnd => buffer.move_word_end(),
            EditorVimOperatorMotion::WordEndBackward => buffer.move_previous_word_end(),
            EditorVimOperatorMotion::WordForward => buffer.move_word_right(),
            EditorVimOperatorMotion::LineColumn
            | EditorVimOperatorMotion::LineColumnStart
            | EditorVimOperatorMotion::LineEnd
            | EditorVimOperatorMotion::LineFirstNonWhitespace
            | EditorVimOperatorMotion::MatchingBracket
            | EditorVimOperatorMotion::ParagraphBackward
            | EditorVimOperatorMotion::ParagraphForward => {}
        }
    }
    let mut end = buffer.cursor();
    buffer.set_single_cursor(start);

    if matches!(
        motion,
        EditorVimOperatorMotion::BigWordEnd | EditorVimOperatorMotion::WordEnd
    ) && end >= start
    {
        end = end.saturating_add(1).min(buffer.len_chars());
    }
    if start == end {
        return None;
    }
    Some(start.min(end)..start.max(end))
}

fn vim_operator_char_find_range(
    buffer: &mut TextBuffer,
    count: usize,
    motion: EditorVimCharFindMotion,
    target: char,
) -> Option<Range<usize>> {
    let start = buffer.cursor();
    if !vim_apply_char_find(buffer, count, motion, target) {
        return None;
    }
    let found = buffer.cursor();
    buffer.set_single_cursor(start);
    if found == start {
        return None;
    }
    if found > start {
        let end = found.saturating_add(1).min(buffer.len_chars());
        (start < end).then_some(start..end)
    } else {
        Some(found..start)
    }
}

fn vim_paragraph_motion_char(buffer: &mut TextBuffer, count: usize, forward: bool) -> usize {
    let original = buffer.cursor();
    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        let target = if forward {
            vim_next_paragraph_line(buffer)
        } else {
            vim_previous_paragraph_line(buffer)
        };
        buffer.set_single_cursor(buffer.line_column_to_char(target, 0));
    }
    let target = buffer.cursor();
    buffer.set_single_cursor(original);
    target
}
