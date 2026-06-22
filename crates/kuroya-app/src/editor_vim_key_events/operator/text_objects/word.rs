use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::super::{VIM_MAX_COUNT, vim_char_at};

pub(super) fn vim_inner_word_range(buffer: &mut TextBuffer, count: usize) -> Option<Range<usize>> {
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

pub(super) fn vim_inner_big_word_range(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    let first = vim_big_word_range_at(buffer, buffer.cursor())?;
    let mut end = first.end;
    for _ in 1..count.clamp(1, VIM_MAX_COUNT) {
        let next = vim_big_word_range_after(buffer, end)?;
        end = next.end.max(end);
    }
    (first.start < end).then_some(first.start..end)
}

pub(super) fn vim_outer_word_range(buffer: &TextBuffer, inner: Range<usize>) -> Range<usize> {
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

fn vim_is_text_object_blank(ch: char) -> bool {
    ch.is_whitespace() && ch != '\n' && ch != '\r'
}
