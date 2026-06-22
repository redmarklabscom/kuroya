use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::super::{EditorVimTextObjectScope, VIM_MAX_COUNT, vim_char_at};

pub(super) fn vim_sentence_text_object_range(
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
