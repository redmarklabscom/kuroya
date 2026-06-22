use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::super::{EditorVimTextObjectScope, VIM_MAX_COUNT, vim_char_at};
use super::vim_line_start_char;

pub(super) fn vim_block_text_object_range(
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

pub(super) fn vim_quote_text_object_range(
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
