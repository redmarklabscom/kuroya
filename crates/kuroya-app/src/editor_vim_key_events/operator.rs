use kuroya_core::TextBuffer;
use std::ops::Range;

use super::{
    EditorVimCaseConversion, EditorVimCharFindMotion, EditorVimNamedRegister,
    EditorVimOperatorMotion, EditorVimRegister, EditorVimRegisterKind, EditorVimTextObjectKind,
    EditorVimTextObjectScope, VIM_MAX_COUNT, vim_apply_char_find, vim_char_at, vim_combined_count,
    vim_convert_case_range, vim_line_column_motion_char, vim_line_first_non_whitespace_char,
    vim_matching_bracket_range, vim_move_previous_big_word_end, vim_next_paragraph_line,
    vim_operator_search_match_range, vim_operator_search_repeat_range,
    vim_operator_search_word_under_cursor_range, vim_previous_paragraph_line,
    vim_toggle_case_range, vim_write_registers,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorVimParagraphRange {
    start_line: usize,
    end_line: usize,
    content: Range<usize>,
}

pub(super) fn vim_apply_operator_motion(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_apply_operator_motion_into_registers(
        buffer,
        operator_count,
        motion_count,
        motion,
        unnamed_register,
        None,
    )
}

pub(super) fn vim_apply_operator_motion_into_named_register(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_apply_operator_motion_into_registers(
        buffer,
        operator_count,
        motion_count,
        motion,
        unnamed_register,
        Some(named_register),
    )
}

fn vim_apply_operator_motion_into_registers(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_operator_motion_range(buffer, count, motion) else {
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

pub(super) fn vim_convert_case_operator_motion(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    conversion: EditorVimCaseConversion,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_operator_motion_range(buffer, count, motion) else {
        return false;
    };
    let cursor = range.start;
    vim_convert_case_range(buffer, range, cursor, conversion)
}

pub(super) fn vim_convert_case_text_object(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    conversion: EditorVimCaseConversion,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_text_object_range(buffer, count, scope, kind) else {
        return false;
    };
    let cursor = range.start;
    vim_convert_case_range(buffer, range, cursor, conversion)
}

pub(super) fn vim_toggle_case_operator_motion(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_operator_motion_range(buffer, count, motion) else {
        return false;
    };
    let cursor = range.start;
    vim_toggle_case_range(buffer, range, cursor)
}

pub(super) fn vim_toggle_case_text_object(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_text_object_range(buffer, count, scope, kind) else {
        return false;
    };
    let cursor = range.start;
    vim_toggle_case_range(buffer, range, cursor)
}

pub(super) fn vim_apply_text_object(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_apply_text_object_into_registers(
        buffer,
        operator_count,
        motion_count,
        scope,
        kind,
        unnamed_register,
        None,
    )
}

pub(super) fn vim_apply_text_object_into_named_register(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_apply_text_object_into_registers(
        buffer,
        operator_count,
        motion_count,
        scope,
        kind,
        unnamed_register,
        Some(named_register),
    )
}

fn vim_apply_text_object_into_registers(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_text_object_range(buffer, count, scope, kind) else {
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

pub(super) fn vim_yank_operator_motion(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_yank_operator_motion_into_registers(
        buffer,
        operator_count,
        motion_count,
        motion,
        unnamed_register,
        None,
    )
}

pub(super) fn vim_yank_operator_motion_into_named_register(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_yank_operator_motion_into_registers(
        buffer,
        operator_count,
        motion_count,
        motion,
        unnamed_register,
        Some(named_register),
    )
}

fn vim_yank_operator_motion_into_registers(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_operator_motion_range(buffer, count, motion) else {
        return false;
    };
    vim_yank_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        named_register,
    )
}

pub(super) fn vim_yank_text_object(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_yank_text_object_into_registers(
        buffer,
        operator_count,
        motion_count,
        scope,
        kind,
        unnamed_register,
        None,
    )
}

pub(super) fn vim_yank_text_object_into_named_register(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_yank_text_object_into_registers(
        buffer,
        operator_count,
        motion_count,
        scope,
        kind,
        unnamed_register,
        Some(named_register),
    )
}

fn vim_yank_text_object_into_registers(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_text_object_range(buffer, count, scope, kind) else {
        return false;
    };
    vim_yank_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        named_register,
    )
}

pub(super) fn vim_text_object_range(
    buffer: &mut TextBuffer,
    count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
) -> Option<Range<usize>> {
    let inner = match kind {
        EditorVimTextObjectKind::Word => vim_inner_word_range(buffer, count),
        EditorVimTextObjectKind::BigWord => vim_inner_big_word_range(buffer, count),
        EditorVimTextObjectKind::Block { open, close } => {
            return vim_block_text_object_range(buffer, count, scope, open, close);
        }
        EditorVimTextObjectKind::Quote { quote } => {
            return vim_quote_text_object_range(buffer, count, scope, quote);
        }
        EditorVimTextObjectKind::Paragraph => {
            return vim_paragraph_text_object_range(buffer, count, scope);
        }
        EditorVimTextObjectKind::Sentence => {
            return vim_sentence_text_object_range(buffer, count, scope);
        }
    }?;
    match scope {
        EditorVimTextObjectScope::Inner => Some(inner),
        EditorVimTextObjectScope::Outer => Some(vim_outer_word_range(buffer, inner)),
    }
}

fn vim_inner_word_range(buffer: &mut TextBuffer, count: usize) -> Option<Range<usize>> {
    let original_cursor = buffer.cursor();
    let first = buffer.word_range_at_cursor()?;
    let mut end = first.end;
    for _ in 1..count.clamp(1, VIM_MAX_COUNT) {
        buffer.set_single_cursor(end);
        buffer.move_word_right();
        end = buffer.cursor().max(end);
    }
    buffer.set_single_cursor(original_cursor);
    (first.start < end).then_some(first.start..end)
}

fn vim_inner_big_word_range(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    let first = vim_big_word_range_at(buffer, buffer.cursor())?;
    let mut end = first.end;
    for _ in 1..count.clamp(1, VIM_MAX_COUNT) {
        let next = vim_big_word_range_after(buffer, end)?;
        end = next.end.max(end);
    }
    (first.start < end).then_some(first.start..end)
}

fn vim_big_word_range_at(buffer: &TextBuffer, cursor: usize) -> Option<Range<usize>> {
    let len = buffer.len_chars();
    if len == 0 {
        return None;
    }

    let cursor = cursor.min(len);
    let word_idx = if cursor < len && !vim_char_at(buffer, cursor)?.is_whitespace() {
        cursor
    } else if cursor > 0 && !vim_char_at(buffer, cursor - 1)?.is_whitespace() {
        cursor - 1
    } else {
        return None;
    };

    let mut start = word_idx;
    while start > 0 && !vim_char_at(buffer, start - 1)?.is_whitespace() {
        start -= 1;
    }

    let mut end = word_idx + 1;
    while end < len && !vim_char_at(buffer, end)?.is_whitespace() {
        end += 1;
    }

    Some(start..end)
}

fn vim_big_word_range_after(buffer: &TextBuffer, after: usize) -> Option<Range<usize>> {
    let len = buffer.len_chars();
    let mut start = after.min(len);
    while start < len && vim_char_at(buffer, start)?.is_whitespace() {
        start += 1;
    }
    if start >= len {
        return None;
    }

    let mut end = start + 1;
    while end < len && !vim_char_at(buffer, end)?.is_whitespace() {
        end += 1;
    }
    Some(start..end)
}

fn vim_outer_word_range(buffer: &TextBuffer, inner: Range<usize>) -> Range<usize> {
    let len = buffer.len_chars();
    let mut end = inner.end.min(len);
    while end < len && vim_char_at(buffer, end).is_some_and(vim_is_text_object_blank) {
        end += 1;
    }
    if end > inner.end {
        return inner.start..end;
    }

    let mut start = inner.start.min(len);
    while start > 0 && vim_char_at(buffer, start - 1).is_some_and(vim_is_text_object_blank) {
        start -= 1;
    }
    start..inner.end
}

fn vim_is_text_object_blank(ch: char) -> bool {
    ch.is_whitespace() && ch != '\n' && ch != '\r'
}

fn vim_block_text_object_range(
    buffer: &TextBuffer,
    count: usize,
    scope: EditorVimTextObjectScope,
    open: char,
    close: char,
) -> Option<Range<usize>> {
    let (open_idx, close_idx) = vim_block_text_object_pair(buffer, count, open, close)?;
    match scope {
        EditorVimTextObjectScope::Inner => {
            let start = open_idx.saturating_add(1);
            (start < close_idx).then_some(start..close_idx)
        }
        EditorVimTextObjectScope::Outer => {
            Some(open_idx..close_idx.saturating_add(1).min(buffer.len_chars()))
        }
    }
}

fn vim_block_text_object_pair(
    buffer: &TextBuffer,
    count: usize,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    let len = buffer.len_chars();
    let cursor = buffer.cursor().min(len);
    let mut stack = Vec::new();
    let mut candidates = Vec::new();
    for idx in 0..len {
        let Some(ch) = vim_char_at(buffer, idx) else {
            continue;
        };
        if ch == open {
            stack.push(idx);
        } else if ch == close
            && let Some(open_idx) = stack.pop()
            && open_idx <= cursor
            && cursor <= idx
        {
            candidates.push((open_idx, idx));
        }
    }

    candidates.sort_by(|left, right| {
        left.1
            .saturating_sub(left.0)
            .cmp(&right.1.saturating_sub(right.0))
            .then(left.0.cmp(&right.0).reverse())
    });
    candidates
        .into_iter()
        .nth(count.clamp(1, VIM_MAX_COUNT) - 1)
}

fn vim_quote_text_object_range(
    buffer: &TextBuffer,
    count: usize,
    scope: EditorVimTextObjectScope,
    quote: char,
) -> Option<Range<usize>> {
    let (open_idx, close_idx) = vim_quote_text_object_pair(buffer, count, quote)?;
    match scope {
        EditorVimTextObjectScope::Inner => {
            let start = open_idx.saturating_add(1);
            (start < close_idx).then_some(start..close_idx)
        }
        EditorVimTextObjectScope::Outer => {
            Some(open_idx..close_idx.saturating_add(1).min(buffer.len_chars()))
        }
    }
}

fn vim_quote_text_object_pair(
    buffer: &TextBuffer,
    count: usize,
    quote: char,
) -> Option<(usize, usize)> {
    let line = buffer.cursor_position().line;
    let line_start = vim_line_start_char(buffer, line);
    let line_end = buffer.line_content_end_char(line);
    let cursor = buffer.cursor().min(line_end);
    let mut open_idx = None;
    let mut candidates = Vec::new();

    for idx in line_start..line_end {
        if vim_char_at(buffer, idx) != Some(quote)
            || vim_quote_char_is_escaped(buffer, idx, line_start)
        {
            continue;
        }

        if let Some(open) = open_idx.take() {
            if open <= cursor && cursor <= idx {
                candidates.push((open, idx));
            }
        } else {
            open_idx = Some(idx);
        }
    }

    candidates
        .into_iter()
        .nth(count.clamp(1, VIM_MAX_COUNT) - 1)
}

fn vim_quote_char_is_escaped(buffer: &TextBuffer, idx: usize, line_start: usize) -> bool {
    let mut slash_count = 0;
    let mut probe = idx;
    while probe > line_start && vim_char_at(buffer, probe - 1) == Some('\\') {
        slash_count += 1;
        probe -= 1;
    }
    slash_count % 2 == 1
}

fn vim_sentence_text_object_range(
    buffer: &TextBuffer,
    count: usize,
    scope: EditorVimTextObjectScope,
) -> Option<Range<usize>> {
    let sentences = vim_sentence_content_ranges(buffer);
    let cursor = buffer.cursor().min(buffer.len_chars());
    let sentence_idx = sentences
        .iter()
        .position(|range| range.start <= cursor && cursor < range.end)
        .or_else(|| sentences.iter().position(|range| cursor < range.start))
        .or_else(|| sentences.len().checked_sub(1))?;
    let count = count.clamp(1, VIM_MAX_COUNT);
    let last_idx = sentence_idx
        .saturating_add(count.saturating_sub(1))
        .min(sentences.len().saturating_sub(1));
    let inner = sentences[sentence_idx].start..sentences[last_idx].end;
    match scope {
        EditorVimTextObjectScope::Inner => Some(inner),
        EditorVimTextObjectScope::Outer => Some(vim_outer_sentence_range(buffer, inner)),
    }
}

fn vim_sentence_content_ranges(buffer: &TextBuffer) -> Vec<Range<usize>> {
    let len = buffer.len_chars();
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < len {
        while start < len && vim_char_at(buffer, start).is_some_and(char::is_whitespace) {
            start += 1;
        }
        if start >= len {
            break;
        }

        let mut end = len;
        let mut idx = start;
        while idx < len {
            let Some(ch) = vim_char_at(buffer, idx) else {
                break;
            };
            if vim_is_sentence_terminator(ch) {
                let mut boundary = idx + 1;
                while boundary < len
                    && vim_char_at(buffer, boundary).is_some_and(vim_is_sentence_closer)
                {
                    boundary += 1;
                }
                if boundary >= len || vim_char_at(buffer, boundary).is_some_and(char::is_whitespace)
                {
                    end = boundary;
                    break;
                }
            }
            idx += 1;
        }

        ranges.push(start..end);
        start = end;
    }
    ranges
}

fn vim_outer_sentence_range(buffer: &TextBuffer, inner: Range<usize>) -> Range<usize> {
    let len = buffer.len_chars();
    let mut end = inner.end.min(len);
    while end < len && vim_char_at(buffer, end).is_some_and(char::is_whitespace) {
        end += 1;
    }
    if end > inner.end {
        return inner.start..end;
    }

    let mut start = inner.start.min(len);
    while start > 0 && vim_char_at(buffer, start - 1).is_some_and(char::is_whitespace) {
        start -= 1;
    }
    start..inner.end
}

fn vim_is_sentence_terminator(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?')
}

fn vim_is_sentence_closer(ch: char) -> bool {
    matches!(ch, '"' | '\'' | ')' | ']' | '}')
}

fn vim_paragraph_text_object_range(
    buffer: &TextBuffer,
    count: usize,
    scope: EditorVimTextObjectScope,
) -> Option<Range<usize>> {
    let paragraphs = vim_paragraph_content_ranges(buffer);
    let cursor = buffer.cursor().min(buffer.len_chars());
    let paragraph_idx = paragraphs
        .iter()
        .position(|paragraph| paragraph.content.start <= cursor && cursor < paragraph.content.end)
        .or_else(|| {
            paragraphs
                .iter()
                .position(|paragraph| cursor < paragraph.content.start)
        })
        .or_else(|| paragraphs.len().checked_sub(1))?;
    let count = count.clamp(1, VIM_MAX_COUNT);
    let last_idx = paragraph_idx
        .saturating_add(count.saturating_sub(1))
        .min(paragraphs.len().saturating_sub(1));
    let first = &paragraphs[paragraph_idx];
    let last = &paragraphs[last_idx];
    let inner = first.content.start..last.content.end;
    match scope {
        EditorVimTextObjectScope::Inner => Some(inner),
        EditorVimTextObjectScope::Outer => Some(vim_outer_paragraph_range(buffer, first, last)),
    }
}

fn vim_paragraph_content_ranges(buffer: &TextBuffer) -> Vec<EditorVimParagraphRange> {
    let line_count = buffer.len_lines();
    let mut ranges = Vec::new();
    let mut line = 0;
    while line < line_count {
        while line < line_count && buffer.line_is_blank(line) {
            line += 1;
        }
        if line >= line_count {
            break;
        }

        let start_line = line;
        while line < line_count && !buffer.line_is_blank(line) {
            line += 1;
        }
        let end_line = line.saturating_sub(1);
        let start = vim_line_start_char(buffer, start_line);
        let end = buffer.line_content_end_char(end_line);
        if start < end {
            ranges.push(EditorVimParagraphRange {
                start_line,
                end_line,
                content: start..end,
            });
        }
    }
    ranges
}

fn vim_outer_paragraph_range(
    buffer: &TextBuffer,
    first: &EditorVimParagraphRange,
    last: &EditorVimParagraphRange,
) -> Range<usize> {
    let mut trailing_line = last.end_line.saturating_add(1);
    while trailing_line < buffer.len_lines() && buffer.line_is_blank(trailing_line) {
        trailing_line += 1;
    }
    if trailing_line > last.end_line.saturating_add(1) {
        return first.content.start..vim_line_start_char(buffer, trailing_line);
    }

    let mut start_line = first.start_line;
    while start_line > 0 && buffer.line_is_blank(start_line - 1) {
        start_line -= 1;
    }
    vim_line_start_char(buffer, start_line)..last.content.end
}

fn vim_line_start_char(buffer: &TextBuffer, line: usize) -> usize {
    if line >= buffer.len_lines() {
        buffer.len_chars()
    } else {
        buffer.line_column_to_char(line, 0)
    }
}

fn vim_operator_motion_range(
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

pub(super) fn vim_delete_range_into_register(
    buffer: &mut TextBuffer,
    range: Range<usize>,
    kind: EditorVimRegisterKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    if !vim_yank_range_into_register(
        buffer,
        range.clone(),
        kind,
        unnamed_register,
        named_register,
    ) {
        return false;
    }
    buffer.set_selection(range.start, range.end);
    buffer.delete_selection_ranges()
}

pub(super) fn vim_yank_range_into_register(
    buffer: &TextBuffer,
    range: Range<usize>,
    kind: EditorVimRegisterKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(text) = buffer.text_range(range) else {
        return false;
    };
    if text.is_empty() {
        return false;
    }
    vim_write_registers(
        unnamed_register,
        named_register,
        EditorVimRegister { text, kind },
    );
    true
}

pub(super) fn vim_delete_to_line_end(buffer: &mut TextBuffer, count: usize) -> bool {
    let Some(range) = vim_delete_to_line_end_range(buffer, count) else {
        return false;
    };
    buffer.set_selection(range.start, range.end);
    buffer.delete_selection_ranges()
}

pub(super) fn vim_delete_to_line_end_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    let Some(range) = vim_delete_to_line_end_range(buffer, count) else {
        return false;
    };
    vim_delete_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        Some(named_register),
    )
}

fn vim_delete_to_line_end_range(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let start = buffer.cursor();
    let start_line = buffer.cursor_position().line;
    let end_line = start_line
        .saturating_add(count.saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    let end = buffer.line_content_end_char(end_line);
    (start < end).then_some(start..end)
}
