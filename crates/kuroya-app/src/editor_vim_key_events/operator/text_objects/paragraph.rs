use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::super::{EditorVimTextObjectScope, VIM_MAX_COUNT};
use super::vim_line_start_char;

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorVimParagraphRange {
    start_line: usize,
    end_line: usize,
    content: Range<usize>,
}

pub(super) fn vim_paragraph_text_object_range(
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
