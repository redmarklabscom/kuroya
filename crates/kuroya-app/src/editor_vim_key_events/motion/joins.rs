use kuroya_core::{TextBuffer, TextEdit};

use super::super::VIM_MAX_COUNT;

pub(in crate::editor_vim_key_events) fn vim_join_lines(
    buffer: &mut TextBuffer,
    count: usize,
) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= buffer.join_lines();
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_join_lines_without_whitespace(
    buffer: &mut TextBuffer,
    count: usize,
) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= vim_join_next_line_without_whitespace(buffer);
    }
    changed
}

fn vim_join_next_line_without_whitespace(buffer: &mut TextBuffer) -> bool {
    let line = buffer.cursor_position().line;
    if line + 1 >= buffer.len_lines() {
        return false;
    }
    if buffer.is_final_newline_line(line + 1) {
        return false;
    }

    let start = buffer.line_content_end_char(line);
    let next_line_start = buffer.line_column_to_char(line + 1, 0);
    let next_indent = buffer
        .line(line + 1)
        .unwrap_or_default()
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    let end = next_line_start
        .saturating_add(next_indent)
        .min(buffer.len_chars());
    if start >= end {
        return false;
    }

    let original_cursor = buffer.cursor();
    let changed = buffer.apply_edits(vec![TextEdit {
        range: start..end,
        inserted: String::new(),
    }]);
    if changed {
        buffer.set_single_cursor(original_cursor.min(buffer.len_chars()));
    }
    changed
}
